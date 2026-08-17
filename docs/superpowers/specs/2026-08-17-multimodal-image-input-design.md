# Multimodal Image Input: Web Chat → Agent → Session

**Date:** 2026-08-17
**Status:** approved

## Overview

Add image input to the web chat (React frontend) end-to-end: users can paste (Ctrl+V), pick, or drag-drop images into the conversation; the agent's model receives them; session history preserves them across compression and resume. An image is a `data:image/...;base64` URL inside the existing `AgentInput.parts` array — no new endpoints, no new wire types, no new storage.

**Verified during brainstorming (live probes against the local model proxy `http://192.168.2.162:31693`):**
- Anthropic-format `/v1/messages` with a base64 `image` block → model answered "Red" correctly ✅
- OpenAI-format `/v1/chat/completions` with an `image_url` data-URL part → model answered "Blue" correctly ✅
- All deployed provider configs (configmaps, `.agents/providers/*.toml`) are anthropic-format against this proxy; no OpenAI-format provider is configured anywhere today.

## Design

### 1. Architecture & Data Flow

One invariant: from browser to model, an image is a data URL in the existing `parts` array. Every system in the path already understands this shape.

```
browser (validate, compress ≤1568px, JPEG q0.85)
  → agent.submit: input.parts = [text, image_url(data:...), ...]     [existing wire]
  → control-plane forwards AgentInput verbatim                        [existing, verified]
  → data-plane → ReActAgent::run_input → MessageContent::MultiPart    [existing]
  → RunContext.add_message → session JSONL stores multipart inline    [existing, serde]
  → SessionContributor → context → provider converter                 [Anthropic existing; OpenAI new]
  → proxy at 31693 → vision model                                     [probe-verified both formats]
```

No protocol changes: `vol-llm-agent-protocol` wire types, control-plane routing, and `AgentInput` serialization are untouched.

### 2. Compression Policy (client-side, browser)

Constants live in `frontend/src/lib/image.ts`:

| Constant | Value |
|---|---|
| `MAX_ORIGINAL_BYTES` | 10 MB (reject originals above) |
| `MAX_IMAGES_PER_MESSAGE` | 4 |
| `MAX_LONG_EDGE` | 1568 px (Anthropic-optimal bucket) |
| `JPEG_QUALITY` | 0.85 |
| `KEEP_AS_IS_THRESHOLD` | 300 KB |

Decision rule (`needsReencode`, pure function):
- Original ≤ 300 KB **and** long edge ≤ 1568 px → send bytes as-is (preserves PNG transparency).
- Otherwise → canvas downscale to ≤ 1568 px long edge, re-encode JPEG q0.85.

Accepted input types: png / jpeg / webp / gif. GIFs that pass through unchanged keep their animation; GIFs that get re-encoded are flattened to their first frame (canvas behavior) — acceptable, vision models treat images as static.

### 3. Changes by File

**Frontend (React, `frontend/src`)**

| File | Change |
|---|---|
| `lib/image.ts` (new) | Pure helpers: type/size validation, `needsReencode` decision, canvas compress → data URL. Typed `ImageError` (`TooLarge` / `UnsupportedType` / `CompressionFailed`). |
| `components/inputs/InputArea.tsx` | `onPaste` (clipboard image files; text paste unaffected), hidden `<input type="file" accept="image/*" multiple>` + paperclip attach button (`data-icon="inline-start"`), drop zone on the textarea container. Chips row with thumbnails + per-image remove. Submit builds `parts = [{type:'text',...}, ...images.map(i => ({type:'image_url', url: i.dataUrl}))]`. Submit blocked while any image is compressing. |
| `types/index.ts` | `UserInput` entry gains optional `images?: string[]` (data URLs). Optional so existing code compiles untouched. |
| `components/panels/ConversationView.tsx` | Render `~96px` rounded thumbnails under user text for `images`. |
| `lib/session-conversion.ts` | Extract `{type:'image_url', image_url:{url}}` parts into `images`, keep text parts in `text` (replaces the current behavior of rendering image parts as the literal string `"image"`). |

**Backend (Rust)**

| File | Change |
|---|---|
| `crates/vol-llm-core/src/message.rs` | Add `MessageContent::display_text()`: `Text` → text; `MultiPart` → text parts joined, one `[image]` marker per image part. |
| `crates/vol-llm-agent/src/react/input.rs` | `AgentInput::display_text()` emits `[image]` per image part (instead of silently dropping). Text-only callers unaffected. |
| `crates/vol-llm-context/src/context_block.rs` | `estimate_tokens` multipart-aware: each image part counts a fixed 1600-token budget (Anthropic's ≤1568px bucket); base64 payload bytes are excluded from the `len()/4` calc. Fixes ~100× overcounting that would trigger premature compression. |
| `crates/vol-session/src/session_contributor.rs` | Summary text (step 4 of `compress()`) uses `MessageContent::display_text()` so image presence is recorded as `[image]` instead of vanishing. |
| `crates/vol-session/src/compressors/position_sample.rs` | Image-bearing messages are exempt from positional sampling — always kept, mirroring `keep_first` (images are deliberately attached, high-value context). |
| `crates/vol-llm-provider/src/openai.rs` | `convert_messages` emits OpenAI vision format for multipart user messages: `content: [{type:"text",text}, {type:"image_url", image_url:{url}}]`, mirroring the Anthropic converter (data URLs and http URLs). |
| `crates/vol-llm-provider/src/openai_streaming.rs` | Same conversion if it duplicates `convert_messages` (verify at implementation time). |

**Untouched (deliberate):**
- `vol-llm-agent-protocol` / control-plane: no wire changes.
- Session stores (`file_store.rs`, `database_store`, `entry.rs`): serde already round-trips `MessageContent::MultiPart` in JSONL.
- Anthropic provider converter: already complete and tested.
- TUI: out of scope (web-only per scope decision).

### 4. Session & Compression Semantics

- Raw history: multipart messages persist inline in session JSONL; replayed verbatim into context on resume — images are re-sent to the model exactly as attached.
- Compression: kept messages (first-N, samples, last, all image-bearing) retain full multipart content. The summary entry is text-only; image presence is recorded as `[image]` markers in the summary text. Messages fully compressed away lose their images the same way they lose their text — accepted and documented limitation; a vision-based image summarizer is explicitly out of scope.
- Token accounting: per-image budget of 1600 tokens keeps `estimate_tokens`/context budgeting stable as images accumulate.

### 5. Error Handling

- Per-image errors (too large / unsupported type / compression failure) → inline error on the image chip; submit proceeds with the valid parts. All images failed and empty text → submit blocked with a visible error.
- Model-service rejection (non-vision model, HTTP 400) → flows through the existing run-failure → conversation `Error` entry path; no new machinery.
- Risk to verify at implementation: WS message-size limits on the control-plane transport must accept ~2 MB frames (4 images × ≤300 KB base64 ≈ 1.6 MB). Check transport config; raise the limit if configured lower.

### 6. Testing

**Rust (unit tests per crate convention):**
- `vol-llm-core`: `display_text()` — text, multipart text+image, image-only.
- `vol-llm-agent`: `display_text()` `[image]` markers.
- `vol-llm-context`: `estimate_tokens` — image budget added once per image, base64 bytes excluded (no blowup), text unchanged.
- `vol-session`: compression summary contains `[image]`; image-bearing messages exempt from sampling; multipart content intact after checkpoint rewrite.
- `vol-llm-provider` (openai): multipart → vision format for data-URL and http-URL images; text-only messages unchanged (regression).

**Frontend (vitest):**
- `lib/image.ts`: `needsReencode` decision table (size/long-edge boundaries), validation error mapping.

**Manual verification:**
- Paste + file-picker attach against the local agent (qwen3.6-plus via anthropic-format provider): agent answers about image content.
- Session resume shows thumbnails; context tab shows a sane token estimate.
- Optional live check: OpenAI-format provider config against the same proxy renders images.

**Gates:** `just cover-gate` ≥ 80% for touched Rust crates; existing text-only tests stay green; frontend `npm run test:run` + `typecheck` clean.

### 7. Success Criteria

1. Paste/attach an image in the web UI → agent answers correctly about the image (local qwen3.6-plus).
2. OpenAI-format provider renders images (unit-tested; probe already confirmed the endpoint accepts them).
3. Session resume shows thumbnails; compression summary records `[image]`; kept image messages are re-sent to the model.
4. Token estimation sane — no ~100× overcount from base64.
5. Text-only behavior unchanged; all gates green.

## Out of Scope

- TUI image input/rendering.
- Server-side image upload endpoint / blob-extracted session storage (revisit only if session bloat matters at scale).
- Image output from tools/assistant (e.g., screenshot tool results) — input-only per requirement.
- Vision-based image summarization during compression.
