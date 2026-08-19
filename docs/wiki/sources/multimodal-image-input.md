---
type: source
source_type: design
date: 2026-08-17
ingested: 2026-08-17
tags: [multimodal, image, frontend, session, provider, verification]
---

# Multimodal Image Input Implementation and Verification

**Authors/Creators:** BestNathan / Claude
**Date:** 2026-08-17
**Link:** `docs/superpowers/specs/2026-08-17-multimodal-image-input-design.md`, `docs/superpowers/plans/2026-08-17-multimodal-image-input.md`

## TL;DR

Web chat users can now attach, paste, or drag-drop images into the React frontend; images are compressed client-side, submitted as multipart `parts` in `agent.submit`, persisted through sessions and compression, converted to provider-native vision formats (Anthropic and OpenAI), and rendered as thumbnails in the conversation. The feature was implemented across vol-llm-core, vol-llm-agent, vol-llm-context, vol-session, vol-llm-provider and the React frontend, then verified end-to-end (image correctly described by the model through the live stack) with all gates green.

## Key Takeaways

- `MessageContent::display_text()` and `AgentInput::display_text()` render image parts as `[image]` markers so text-only consumers (TUI, session summary, event input) never see raw base64.
- Token estimation is multipart-aware: each image part costs a fixed `IMAGE_TOKEN_BUDGET = 1600` instead of JSON-length/4 on the base64 blob.
- Session compression preserves images: summary messages carry `[image]` markers and position sampling exempts image-bearing messages from being dropped.
- OpenAI provider converts multipart messages into vision content arrays (`image_url`), completing image support for both provider families (Anthropic conversion already existed).
- Frontend enforces: max 4 images per message, 10MB original cap, client-side JPEG compression (long edge 1568px, quality 0.85), pass-through under 300KB.
- WS frame-size limits: no explicit limit is configured anywhere; axum 0.7.9 / tungstenite defaults (64 MiB message, 16 MiB frame) are far above the ~1.6MB worst case (4 × 300KB × 4/3 base64), so no change was needed.
- End-to-end verification through the live stack (`just web-backend` + WS client): a 32x32 red PNG submitted with the question "Reply with exactly the word RED" produced the answer "RED", and a follow-up question in a resumed session ("what color was the image...") was answered "Red".

## Detailed Summary

### Wire format

The frontend submits `agent.submit` with a structured input:

```json
{"input": {"parts": [
  {"type": "text", "text": "..."},
  {"type": "image_url", "url": "data:image/png;base64,..."}
], "metadata": {"session_id": "..."}}, "target": "general-purpose"}
```

`AgentInput` deserializes from either a legacy string or this structured object; `to_message_content()` converts to `MessageContent::Text` for pure text or `MultiPart` otherwise. The wire `ContentPart` shape (`{"type":"image","image_url":{...}}`) is what gets persisted in session entries.

### Backend crates

- **vol-llm-core** (`src/message.rs`): `MessageContent::display_text()` joins parts with newlines, rendering each image part as `[image]`; four unit tests cover plain/multipart/image-only/empty.
- **vol-llm-agent** (`src/react/input.rs`): `AgentInput::display_text()` (used as `text_content()`) marks `InputPart::ImageUrl` as `[image]`; tests cover text-only and image-only rendering plus the legacy-string deserialization path.
- **vol-llm-context** (`src/context_block.rs`): `estimate_tokens()` gives image parts a fixed `IMAGE_TOKEN_BUDGET = 1600`; tests assert text/4 + budget for one image and 2× budget for two images.
- **vol-session** (`src/session_contributor.rs`, `src/compressors/position_sample.rs`): compression summary messages embed `[image]` markers for image-bearing messages, and position sampling exempts them from removal so images survive compression and stay re-sendable.
- **vol-llm-provider** (`src/openai.rs`): multipart user content converts to OpenAI Chat Completions vision content arrays (text + `image_url` parts), matching the existing Anthropic conversion.

### Frontend

`frontend/src/lib/image.ts` (validation + compression), `components/inputs/InputArea.tsx` (paperclip attach, Ctrl+V paste, drag-drop, chips with remove button, submit guard), and conversation rendering (thumbnail in history, re-rendered from session on reload). 137 frontend unit tests pass, including the new `image.test.ts`, `input-area.test.ts`, and session-conversion tests.

### Verification (Task 9)

- Gate sweep: `cargo test` for the 5 touched crates all green (vol-llm-agent 185, vol-llm-core 41, vol-llm-context 17, vol-llm-provider 44, vol-session 84 unit + integration); `./scripts/check-no-doc-tests.sh` passes; frontend `test:run` 137/137, `typecheck` and `lint` (0 errors) pass; production build succeeds.
- CI aggregate coverage (the binding gate, quality.yml): `cargo llvm-cov` over the 11 core crates = 81.29% line ≥ 80% — PASS. Per-crate self-coverage: vol-llm-agent 84.09%, vol-session 89.27% pass; vol-llm-core 62.93%, vol-llm-context 78.10%, vol-llm-provider 63.31% fail — pre-existing debt (never a CI gate; the aggregate gate passed on main before and after the feature).
- WS frame size: `rg max_message_size|max_frame_size|message_size` in vol-llm-agent-protocol and vol-agent-server finds nothing; both server upgrades (`transport/ws.rs` `/ws`, `jsonrpc/server.rs` `/custom/ws`) use plain axum `WebSocketUpgrade::on_upgrade`, and client connects use `tokio_tungstenite::connect_async` — all default limits (axum 0.7.9 `WebSocketConfig` default: max_message_size 64 MiB, max_frame_size 16 MiB). Recorded as "no explicit limit".
- End-to-end: image-bearing `agent.submit` through the running stack accepted; the model (qwen3.6-plus via local proxy http://192.168.2.162:31693) saw the image and answered correctly; session file persisted the image parts; `session.resume` restored them; follow-up about the image answered correctly; text-only submit behaved unchanged; context estimate for the image conversation (session `e2e-task9-image`, 14 messages, measured on the current code) = **8682 session tokens / 12138 total** (system 26 + skills 3430 + file 0 + session 8682; raw `agent.context_config` output saved at `/tmp/task9-context-config-final.txt`) — low thousands for a fresh image conversation, not hundreds of thousands. An earlier probe against the pre-feature binary (10 messages, no per-image budget) read 707 session / 4249 total; it is superseded by the current-code figure.

## Entities Mentioned

- [[vol-llm-core-crate]]: `MessageContent::display_text()` with `[image]` markers
- [[vol-llm-agent-crate]]: `AgentInput::display_text()` / structured input parts
- [[vol-llm-provider-crate]]: OpenAI vision content arrays
- [[vol-session]]: images kept through compression
- [[vol-llm-agent-protocol-crate]]: wire `agent.submit` structured input; no WS frame-size limit configured

## Concepts Covered

- [[agentinput-multimodal-run]]: display_text image markers, structured wire input
- [[session-compression]]: summary markers and sampling exemption keep images
- [[session-contributor]]: image-aware compression behavior
- [[context-builder]]: multipart estimate_tokens with per-image budget (1600)

## Notes

- The browser-interaction steps of the plan (paste/attach/drag-drop in a real browser, Context-tab rendering) remain manual verification; the API-level paths they exercise were verified over the live WS stack.
- `crates/vol-llm-tools-builtin/web-fetch/.vol/` is an untracked local artifact dir, unrelated to this feature.
- Follow-up (2026-08-19): session detail overlay now renders image parts, thumbnails open a lightbox, and the Attach trigger moved to the CapabilityBar — see [[frontend-image-session-lightbox]].
