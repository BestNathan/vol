# Multimodal Image Input Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users paste/attach images in the web chat; the agent's model sees them, and session history preserves them across compression and resume.

**Architecture:** An image is a `data:image/...;base64` URL inside the existing `AgentInput.parts` array — no new endpoints, wire types, or storage. The browser validates and compresses (≤1568px long edge, JPEG q0.85); the existing anthropic-format provider path already converts multipart (probe-verified against the local proxy at `192.168.2.162:31693`); this plan adds the OpenAI-format conversion, `[image]` display markers, multipart-aware token estimation, session-compression image support, and the frontend attach/paste/render UI.

**Tech Stack:** Rust workspace (vol-llm-core / vol-llm-agent / vol-llm-context / vol-session / vol-llm-provider), React 18 + TypeScript + Tailwind v4 + shadcn/ui (frontend/), vitest (`frontend/tests/unit/`), cargo test.

**Spec:** `docs/superpowers/specs/2026-08-17-multimodal-image-input-design.md`

## Global Constraints

- Coverage ≥ 80% per touched crate: gate with `just cover-gate <crate> 80`. Exception crates: `main.rs`, `app.rs`, `health.rs`.
- No doc tests: unit tests as `#[cfg(test)]` mods; doc code examples must use ```text. Check with `./scripts/check-no-doc-tests.sh`.
- Every new `pub fn` needs at least one test.
- Frontend shadcn/ui conventions: `flex flex-col gap-4` (never `space-y-*`), icons in buttons use `data-icon="inline-start"`, semantic tokens (`bg-primary`, `text-muted-foreground`) never raw colors, `cn()` for conditional classes, `Dialog`/`Sheet` always has a Title.
- Commit style: conventional commits (`feat(vol-llm-core): ...`), end each message with `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Do NOT use git worktrees (user preference). Work on the current branch.
- Constants (copy verbatim from spec): `MAX_ORIGINAL_BYTES=10MB`, `MAX_IMAGES_PER_MESSAGE=4`, `MAX_LONG_EDGE=1568`, `JPEG_QUALITY=0.85`, `KEEP_AS_IS_THRESHOLD=300KB`, per-image token budget `1600`.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/vol-llm-core/src/message.rs` | `MessageContent::display_text()` — multipart-aware text with `[image]` markers |
| `crates/vol-llm-agent/src/react/input.rs` | `AgentInput::display_text()` — same `[image]` marker behavior |
| `crates/vol-llm-context/src/context_block.rs` | `estimate_tokens` — fixed per-image budget, base64 excluded |
| `crates/vol-session/src/session_contributor.rs` | compression summary uses `display_text()` |
| `crates/vol-session/src/compressors/position_sample.rs` | image-bearing messages exempt from sampling |
| `crates/vol-llm-provider/src/openai.rs` | `convert_messages` emits OpenAI vision format for multipart |
| `frontend/src/lib/image.ts` (new) | validation, re-encode decision (pure), canvas compression (DOM) |
| `frontend/tests/unit/image.test.ts` (new) | unit tests for the pure logic |
| `frontend/src/types/index.ts` | `UserInput` entry gains `images?: string[]` |
| `frontend/src/components/inputs/InputArea.tsx` | paste/pick/drop, chips, submit with image parts |
| `frontend/src/components/panels/ConversationView.tsx` | thumbnail rendering for `UserInput.images` |
| `frontend/src/lib/session-conversion.ts` | extract image parts from wire multipart |
| `frontend/tests/unit/session-conversion.test.ts` (new) | unit tests for the extraction |

---

### Task 1: `MessageContent::display_text()` in vol-llm-core

**Files:**
- Modify: `crates/vol-llm-core/src/message.rs` (add method after `as_str`, tests in existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing new.
- Produces: `impl MessageContent { pub fn display_text(&self) -> String }` — `Text(s)` → `s.clone()`; `MultiPart(parts)` → parts mapped to `text` (Text) or `"[image]"` (Image), joined with `"\n"`. Empty MultiPart → `""`. Used by Tasks 2 (reference behavior) and 4.

- [ ] **Step 1: Write the failing tests**

Append to the existing `#[cfg(test)] mod tests` in `crates/vol-llm-core/src/message.rs`:

```rust
    #[test]
    fn test_display_text_plain() {
        let content = MessageContent::Text("hello".to_string());
        assert_eq!(content.display_text(), "hello");
    }

    #[test]
    fn test_display_text_multipart_marks_images() {
        let content = MessageContent::MultiPart(vec![
            ContentPart::Text { text: "before".to_string() },
            ContentPart::Image { image_url: ImageUrl { url: "data:image/png;base64,AAAA".to_string(), detail: None } },
            ContentPart::Text { text: "after".to_string() },
        ]);
        assert_eq!(content.display_text(), "before\n[image]\nafter");
    }

    #[test]
    fn test_display_text_image_only() {
        let content = MessageContent::MultiPart(vec![ContentPart::Image {
            image_url: ImageUrl { url: "https://example.test/a.png".to_string(), detail: None },
        }]);
        assert_eq!(content.display_text(), "[image]");
    }

    #[test]
    fn test_display_text_empty_multipart() {
        let content = MessageContent::MultiPart(vec![]);
        assert_eq!(content.display_text(), "");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-core display_text`
Expected: FAIL — compile error, `display_text` not found.

- [ ] **Step 3: Implement**

In `crates/vol-llm-core/src/message.rs`, inside `impl MessageContent`, after `as_str()`:

```rust
    /// Text representation including image markers.
    /// Multi-part content renders each image part as `[image]`, joined with newlines.
    pub fn display_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::MultiPart(parts) => parts
                .iter()
                .map(|part| match part {
                    ContentPart::Text { text } => text.as_str(),
                    ContentPart::Image { .. } => "[image]",
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-core`
Expected: PASS (all existing + new tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-core/src/message.rs
git commit -m "feat(vol-llm-core): add MessageContent::display_text with [image] markers

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: `AgentInput::display_text()` `[image]` markers in vol-llm-agent

**Files:**
- Modify: `crates/vol-llm-agent/src/react/input.rs:126-135` (the `display_text` method), tests in the existing `#[cfg(test)] mod tests` of the same file.

**Interfaces:**
- Consumes: nothing new (behavior mirrors Task 1).
- Produces: `AgentInput::display_text()` — image parts render as `"[image]"` instead of being dropped. Signature unchanged; text-only inputs are byte-identical to before.

- [ ] **Step 1: Write the failing tests**

Append to the existing tests module in `crates/vol-llm-agent/src/react/input.rs`:

```rust
    #[test]
    fn test_display_text_marks_images() {
        let input = AgentInput::new()
            .text_part("look")
            .image_url("data:image/png;base64,AAAA");
        assert_eq!(input.display_text(), "look\n[image]");
    }

    #[test]
    fn test_display_text_image_only() {
        let input = AgentInput::new().image_url("https://example.test/a.png");
        assert_eq!(input.display_text(), "[image]");
    }

    #[test]
    fn test_display_text_text_only_unchanged() {
        let input = AgentInput::text("hello");
        assert_eq!(input.display_text(), "hello");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-agent display_text`
Expected: FAIL — image parts dropped (image_only asserts `""`, gets `"[image]"`).

- [ ] **Step 3: Implement**

Replace the `display_text` body (currently `filter_map` that drops image parts) with:

```rust
    pub fn display_text(&self) -> String {
        self.parts
            .iter()
            .map(|part| match part {
                InputPart::Text { text } => text.as_str(),
                InputPart::ImageUrl { .. } => "[image]",
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-agent`
Expected: PASS (existing `AgentInput` tests unaffected — text-only inputs unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/input.rs
git commit -m "feat(vol-llm-agent): display [image] markers in AgentInput::display_text

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Multipart-aware `estimate_tokens` in vol-llm-context

**Files:**
- Modify: `crates/vol-llm-context/src/context_block.rs:1` (imports) and `:103-108` (`estimate_tokens`), tests in the existing `#[cfg(test)] mod tests`.

**Interfaces:**
- Consumes: `ContentPart`, `MessageContent` from `vol_llm_core`.
- Produces: `pub const IMAGE_TOKEN_BUDGET: usize = 1600;` and updated `estimate_tokens(msg: &Message) -> usize` — multipart messages count `sum(text.len()/4) + 1600 * image_count`; all other messages unchanged (`json.len()/4`).

- [ ] **Step 1: Write the failing tests**

Append to the existing tests module in `crates/vol-llm-context/src/context_block.rs`:

```rust
    #[test]
    fn test_estimate_tokens_multipart_image_fixed_budget() {
        use vol_llm_core::{ContentPart, ImageUrl, MessageContent};
        // A ~200KB base64 payload must NOT inflate the estimate.
        let big_data_url = format!("data:image/png;base64,{}", "A".repeat(200_000));
        let msg = Message::user(MessageContent::MultiPart(vec![
            ContentPart::Text { text: "what is this".to_string() },
            ContentPart::Image { image_url: ImageUrl { url: big_data_url, detail: None } },
        ]));
        let tokens = estimate_tokens(&msg);
        assert_eq!(tokens, "what is this".len() / 4 + IMAGE_TOKEN_BUDGET);
    }

    #[test]
    fn test_estimate_tokens_two_images() {
        use vol_llm_core::{ContentPart, ImageUrl, MessageContent};
        let msg = Message::user(MessageContent::MultiPart(vec![
            ContentPart::Image { image_url: ImageUrl { url: "https://e.test/a.png".to_string(), detail: None } },
            ContentPart::Image { image_url: ImageUrl { url: "https://e.test/b.png".to_string(), detail: None } },
        ]));
        assert_eq!(estimate_tokens(&msg), 2 * IMAGE_TOKEN_BUDGET);
    }

    #[test]
    fn test_estimate_tokens_text_unchanged() {
        let msg = Message::user("hello world");
        let expected = serde_json::to_string(&msg).unwrap().len() / 4;
        assert_eq!(estimate_tokens(&msg), expected);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-context estimate_tokens`
Expected: FAIL — current implementation returns `json.len()/4` (~50,000 for the first test).

- [ ] **Step 3: Implement**

Change the import at the top of `crates/vol-llm-context/src/context_block.rs` to:

```rust
use vol_llm_core::{ContentPart, Message, MessageContent};
```

Replace `estimate_tokens` (and its doc comment) with:

```rust
/// Fixed token budget per image part (Anthropic's ≤1568px bucket ≈ 1600 tokens).
/// Base64 payload bytes are deliberately excluded from estimation — they
/// overcount real vision cost by ~100x and would trigger premature compression.
pub const IMAGE_TOKEN_BUDGET: usize = 1600;

/// Estimate token count for a message.
/// Multi-part content: text parts count as `len()/4`, each image part counts
/// `IMAGE_TOKEN_BUDGET`. Everything else uses JSON length / 4 as a rough approximation.
pub fn estimate_tokens(msg: &Message) -> usize {
    match &msg.content {
        Some(MessageContent::MultiPart(parts)) => parts
            .iter()
            .map(|part| match part {
                ContentPart::Text { text } => text.len() / 4,
                ContentPart::Image { .. } => IMAGE_TOKEN_BUDGET,
            })
            .sum(),
        _ => {
            let json = serde_json::to_string(msg).unwrap_or_default();
            json.len() / 4
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-context`
Expected: PASS (existing TokenBudget/AttentionAnchor tests untouched).

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-context/src/context_block.rs
git commit -m "feat(vol-llm-context): multipart-aware estimate_tokens with per-image budget

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Session compression supports images (vol-session)

**Files:**
- Modify: `crates/vol-session/src/session_contributor.rs:106-112` (summary building), tests in its `#[cfg(test)] mod tests`.
- Modify: `crates/vol-session/src/compressors/position_sample.rs` (sampling exemption), tests in its `#[cfg(test)] mod tests`.

**Interfaces:**
- Consumes: `MessageContent::display_text()` from Task 1.
- Produces: `compress()` summaries contain `[image]` markers; `PositionSampleCompressor` never samples away an image-bearing message.

- [ ] **Step 1: Write the failing tests**

In `crates/vol-session/src/session_contributor.rs` tests module, append:

```rust
    #[tokio::test]
    async fn test_compress_summary_includes_image_marker() {
        use vol_llm_core::{ContentPart, ImageUrl, MessageContent};
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);
        let multipart = SessionMessage::new(
            session.id.clone(),
            Message::user(MessageContent::MultiPart(vec![
                ContentPart::Text { text: "look".to_string() },
                ContentPart::Image {
                    image_url: ImageUrl { url: "data:image/png;base64,AAAA".to_string(), detail: None },
                },
            ])),
        );
        session.add_message(multipart).await.unwrap();
        let session = Arc::new(tokio::sync::Mutex::new(session));
        let mut contributor =
            SessionContributor::new(session.clone(), 10, AttentionAnchor::Middle(0));
        contributor.compress().await;

        let msgs = session.lock().await.get_messages().await.unwrap();
        let summary_text = msgs
            .iter()
            .filter(|m| m.message.role == vol_llm_core::MessageRole::System)
            .filter_map(|m| m.message.content.as_ref().map(vol_llm_core::MessageContent::display_text))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(summary_text.contains("[image]"), "summary lost image marker: {summary_text}");
    }

    #[tokio::test]
    async fn test_compress_keeps_multipart_content_after_checkpoint() {
        use vol_llm_core::{ContentPart, ImageUrl, MessageContent};
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);
        for i in 0..10 {
            let content = if i == 7 {
                MessageContent::MultiPart(vec![ContentPart::Image {
                    image_url: ImageUrl { url: "https://e.test/x.png".to_string(), detail: None },
                }])
            } else {
                MessageContent::Text(format!("msg-{i}"))
            };
            session.add_message(SessionMessage::new(session.id.clone(), Message::user(content))).await.unwrap();
        }
        let session = Arc::new(tokio::sync::Mutex::new(session));
        let mut contributor =
            SessionContributor::new(session.clone(), 10, AttentionAnchor::Middle(0));
        contributor.compress().await;

        let msgs = session.lock().await.get_messages().await.unwrap();
        assert!(
            msgs.iter().any(|m| matches!(
                &m.message.content,
                Some(MessageContent::MultiPart(parts)) if parts.iter().any(|p| matches!(p, ContentPart::Image { .. }))
            )),
            "image-bearing message was lost after compression"
        );
    }
```

In `crates/vol-session/src/compressors/position_sample.rs` tests module, append:

```rust
    #[tokio::test]
    async fn test_image_messages_exempt_from_sampling() {
        use vol_llm_core::{ContentPart, ImageUrl, MessageContent};
        let compressor = PositionSampleCompressor::new(3, 5);
        let mut messages: Vec<_> = (1..=10).map(|i| make_msg(&i.to_string())).collect();
        // Message 7 carries an image and would otherwise be sampled out
        // (rest-index 3 is not a multiple of sample_every=5).
        messages[6] = SessionMessage::new(
            "test".to_string(),
            Message::user(MessageContent::MultiPart(vec![ContentPart::Image {
                image_url: ImageUrl { url: "https://e.test/7.png".to_string(), detail: None },
            }])),
        );
        let result = compressor.compress(messages).await;
        // Base sampling keeps [1,2,3] + [4,9] + last 10; image msg 7 must be added.
        let texts: Vec<String> = result
            .iter()
            .map(|m| m.message.content.as_ref().unwrap().display_text())
            .collect();
        assert!(texts.contains(&"[image]".to_string()), "image message was sampled away: {texts:?}");
        assert_eq!(result.len(), 7);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-session compress`
Expected: FAIL — summary has no `[image]` marker; sampling drops message 7.

- [ ] **Step 3: Implement**

In `crates/vol-session/src/session_contributor.rs`, replace lines 106-112 (summary building) with:

```rust
        // 4. Build summary text from compressed messages (images rendered as [image])
        let summary = compressed
            .iter()
            .filter_map(|m| m.message.content.as_ref())
            .map(vol_llm_core::MessageContent::display_text)
            .collect::<Vec<_>>()
            .join("\n");
```

In `crates/vol-session/src/compressors/position_sample.rs`, add a helper and use it in the sampling loop:

```rust
/// Image-bearing messages are deliberately attached, high-value context:
/// positional sampling must never drop them (mirrors `keep_first`).
fn has_image_part(msg: &SessionMessage) -> bool {
    matches!(
        &msg.message.content,
        Some(vol_llm_core::MessageContent::MultiPart(parts))
            if parts.iter().any(|p| matches!(p, vol_llm_core::ContentPart::Image { .. }))
    )
}
```

Change the sampling loop condition from:

```rust
            if i % self.sample_every == 0 {
                result.push(msg.clone());
            }
```

to:

```rust
            if i % self.sample_every == 0 || has_image_part(msg) {
                result.push(msg.clone());
            }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-session`
Expected: PASS (existing compressor/contributor tests unchanged).

- [ ] **Step 5: Coverage gate**

Run: `just cover-gate vol-session 80`
Expected: PASS ≥ 80%.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-session/src/session_contributor.rs crates/vol-session/src/compressors/position_sample.rs
git commit -m "feat(vol-session): keep images through compression (summary markers, sampling exemption)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: OpenAI provider vision conversion

**Files:**
- Modify: `crates/vol-llm-provider/src/openai.rs:11-15` (imports) and `:63-135` (`convert_messages`), tests in the existing `#[cfg(test)] mod tests` (there is a `make_provider()` helper around line 617).

**Interfaces:**
- Consumes: `ContentPart`, `MessageContent` from `vol_llm_core`.
- Produces: `fn convert_content(&self, content: Option<&MessageContent>) -> serde_json::Value` — `MultiPart` → OpenAI vision array `[{type:"text",text}, {type:"image_url", image_url:{url}}]`; everything else → `json!(content.map(as_str).unwrap_or(""))` (unchanged behavior).

- [ ] **Step 1: Write the failing tests**

Append to the existing tests module in `crates/vol-llm-provider/src/openai.rs`:

```rust
    #[test]
    fn test_convert_messages_multipart_data_url_image() {
        use vol_llm_core::{ContentPart, ImageUrl, MessageContent};
        let provider = make_provider();
        let messages = vec![Message::user(MessageContent::MultiPart(vec![
            ContentPart::Text { text: "What color?".to_string() },
            ContentPart::Image {
                image_url: ImageUrl {
                    url: "data:image/png;base64,QUJD".to_string(),
                    detail: None,
                },
            },
        ]))];
        let result = provider.convert_messages(&messages);
        assert_eq!(result.len(), 1);
        let content = result[0]["content"].as_array().expect("content must be a vision array");
        assert_eq!(content.len(), 2);
        assert_eq!(content[0], serde_json::json!({"type": "text", "text": "What color?"}));
        assert_eq!(
            content[1],
            serde_json::json!({"type": "image_url", "image_url": {"url": "data:image/png;base64,QUJD"}})
        );
    }

    #[test]
    fn test_convert_messages_multipart_http_url_image() {
        use vol_llm_core::{ContentPart, ImageUrl, MessageContent};
        let provider = make_provider();
        let messages = vec![Message::user(MessageContent::MultiPart(vec![ContentPart::Image {
            image_url: ImageUrl { url: "https://example.test/chart.png".to_string(), detail: None },
        }]))];
        let result = provider.convert_messages(&messages);
        assert_eq!(
            result[0]["content"][0],
            serde_json::json!({"type": "image_url", "image_url": {"url": "https://example.test/chart.png"}})
        );
    }

    #[test]
    fn test_convert_messages_text_only_unchanged() {
        let provider = make_provider();
        let messages = vec![Message::user("Hello")];
        let result = provider.convert_messages(&messages);
        assert_eq!(result[0]["content"], "Hello");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-provider convert_messages_multipart`
Expected: FAIL — current output is `content: ""` (MultiPart → `as_str()`).

- [ ] **Step 3: Implement**

Extend the `vol_llm_core` import in `crates/vol-llm-provider/src/openai.rs` to include `ContentPart, MessageContent`:

```rust
use vol_llm_core::{
    ContentPart, ConversationRequest, ConversationResponse, FinishReason, ImageUrl, LLMClient,
    LLMError, LLMProvider, Message, MessageContent, MessageRole, Result, StreamReceiver,
    StreamingSession, SupportedParam, TokenUsage, ToolCall, ToolDefinition,
};
```

Add a `convert_content` helper inside `impl OpenaiProvider` (before `convert_messages`):

```rust
    /// Convert message content to the OpenAI wire shape.
    /// Multi-part user content becomes OpenAI's vision content array; text
    /// content passes through unchanged.
    fn convert_content(&self, content: Option<&MessageContent>) -> serde_json::Value {
        match content {
            Some(MessageContent::MultiPart(parts)) => {
                let blocks: Vec<serde_json::Value> = parts
                    .iter()
                    .map(|part| match part {
                        ContentPart::Text { text } => json!({
                            "type": "text",
                            "text": text,
                        }),
                        ContentPart::Image { image_url } => json!({
                            "type": "image_url",
                            "image_url": { "url": image_url.url },
                        }),
                    })
                    .collect();
                json!(blocks)
            }
            other => json!(other.map(MessageContent::as_str).unwrap_or("")),
        }
    }
```

In `convert_messages`, replace the `MessageRole::User` arm body so the user message uses `convert_content`:

```rust
                MessageRole::User => {
                    json!({
                        "role": "user",
                        "content": self.convert_content(msg.content.as_ref()),
                    })
                }
```

Leave `System`, `Assistant`, and `Tool` arms as they are (multipart only originates from user input).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-provider`
Expected: PASS (existing `test_convert_messages_*` tests are text-only and unchanged).

- [ ] **Step 5: Coverage gate**

Run: `just cover-gate vol-llm-provider 80`
Expected: PASS ≥ 80%.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-provider/src/openai.rs
git commit -m "feat(vol-llm-provider): OpenAI vision content arrays for multipart messages

Co-Authored-By: Claude <noreply@anthropic.com>"
```

Note: `crates/vol-llm-provider/src/openai_streaming.rs` is a stream parser only (no message conversion) — verified, no changes needed there.

---

### Task 6: Frontend `lib/image.ts` — validation, compression decision, canvas compress

**Files:**
- Create: `frontend/src/lib/image.ts`
- Test: `frontend/tests/unit/image.test.ts`

**Interfaces:**
- Consumes: nothing (browser `File`, `Image`, `canvas` APIs).
- Produces (used by Task 7):
  - `export class ImageError extends Error { constructor(public kind: ImageErrorKind, message: string) }`
  - `export type ImageErrorKind = 'TooLarge' | 'UnsupportedType' | 'CompressionFailed'`
  - `export const MAX_ORIGINAL_BYTES = 10 * 1024 * 1024`
  - `export const MAX_IMAGES_PER_MESSAGE = 4`
  - `export const MAX_LONG_EDGE = 1568`
  - `export const JPEG_QUALITY = 0.85`
  - `export const KEEP_AS_IS_THRESHOLD = 300 * 1024`
  - `export function validateImageFile(file: File): void` — throws `ImageError`
  - `export function needsReencode(bytes: number, width: number, height: number): boolean`
  - `export function compressImageFile(file: File): Promise<string>` — resolves to a data URL

- [ ] **Step 1: Write the failing tests**

Create `frontend/tests/unit/image.test.ts` (unit tests live in `frontend/tests/unit/`, imported via the `@` alias; a plain object cast to `File` suffices since only `.size`/`.type` are read):

```ts
// frontend/tests/unit/image.test.ts
import { describe, it, expect } from 'vitest'
import {
  ImageError,
  KEEP_AS_IS_THRESHOLD,
  MAX_LONG_EDGE,
  MAX_ORIGINAL_BYTES,
  needsReencode,
  validateImageFile,
} from '@/lib/image'

function fakeFile(size: number, type: string): File {
  return { size, type } as File
}

describe('validateImageFile', () => {
  it('accepts supported image types under the size cap', () => {
    expect(() => validateImageFile(fakeFile(1024, 'image/png'))).not.toThrow()
    expect(() => validateImageFile(fakeFile(1024, 'image/jpeg'))).not.toThrow()
    expect(() => validateImageFile(fakeFile(1024, 'image/webp'))).not.toThrow()
    expect(() => validateImageFile(fakeFile(1024, 'image/gif'))).not.toThrow()
  })

  it('rejects files over 10MB', () => {
    expect(() => validateImageFile(fakeFile(MAX_ORIGINAL_BYTES + 1, 'image/png'))).toThrowError(
      expect.objectContaining({ kind: 'TooLarge' }) as unknown as Error,
    )
  })

  it('rejects non-image types', () => {
    expect(() => validateImageFile(fakeFile(1024, 'text/plain'))).toThrowError(
      expect.objectContaining({ kind: 'UnsupportedType' }) as unknown as Error,
    )
  })
})

describe('needsReencode', () => {
  it('keeps small images under the long-edge cap as-is', () => {
    expect(needsReencode(KEEP_AS_IS_THRESHOLD, 1000, 1000)).toBe(false)
  })

  it('re-encodes images over the long-edge cap', () => {
    expect(needsReencode(KEEP_AS_IS_THRESHOLD - 1, MAX_LONG_EDGE + 1, 100)).toBe(true)
  })

  it('re-encodes images over the byte threshold', () => {
    expect(needsReencode(KEEP_AS_IS_THRESHOLD + 1, 100, 100)).toBe(true)
  })
})
```

Note: `expect.toThrowError(expect.objectContaining(...))` matches the thrown `ImageError`'s `kind` property; if the matcher signature complains, use `toThrow(ImageError)` plus a separate `try/catch` asserting `(e as ImageError).kind` — pick whichever vitest accepts; the assertions above are the intent.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd frontend && npm run test:run -- image.test.ts`
Expected: FAIL — module `@/lib/image` does not exist.

- [ ] **Step 3: Implement**

Create `frontend/src/lib/image.ts`:

```ts
// frontend/src/lib/image.ts
// Client-side image validation + compression. Pure decision logic lives here
// (unit-tested); the canvas/file plumbing is exercised via manual testing.
export type ImageErrorKind = 'TooLarge' | 'UnsupportedType' | 'CompressionFailed'

export class ImageError extends Error {
  constructor(
    public kind: ImageErrorKind,
    message: string,
  ) {
    super(message)
    this.name = 'ImageError'
  }
}

export const MAX_ORIGINAL_BYTES = 10 * 1024 * 1024
export const MAX_IMAGES_PER_MESSAGE = 4
export const MAX_LONG_EDGE = 1568
export const JPEG_QUALITY = 0.85
export const KEEP_AS_IS_THRESHOLD = 300 * 1024

const SUPPORTED_TYPES = ['image/png', 'image/jpeg', 'image/webp', 'image/gif']

/** Throw ImageError unless the file is a supported image type under the size cap. */
export function validateImageFile(file: File): void {
  if (!SUPPORTED_TYPES.includes(file.type)) {
    throw new ImageError('UnsupportedType', `Unsupported image type: ${file.type || 'unknown'}`)
  }
  if (file.size > MAX_ORIGINAL_BYTES) {
    throw new ImageError('TooLarge', 'Image exceeds the 10MB limit')
  }
}

/** True when the image must be downscaled/re-encoded before sending. */
export function needsReencode(bytes: number, width: number, height: number): boolean {
  return bytes > KEEP_AS_IS_THRESHOLD || Math.max(width, height) > MAX_LONG_EDGE
}

/** Load the file's pixel dimensions without rendering it into the DOM. */
function loadImage(file: File): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file)
    const img = new Image()
    img.onload = () => {
      URL.revokeObjectURL(url)
      resolve(img)
    }
    img.onerror = () => {
      URL.revokeObjectURL(url)
      reject(new ImageError('CompressionFailed', 'Could not decode the image'))
    }
    img.src = url
  })
}

function toDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(new ImageError('CompressionFailed', 'Could not read the image'))
    reader.readAsDataURL(file)
  })
}

/**
 * Validate + compress an image file to a data URL.
 * Small images (<=300KB, <=1568px long edge) pass through unchanged (keeps
 * PNG transparency); larger ones are downscaled and re-encoded as JPEG.
 */
export async function compressImageFile(file: File): Promise<string> {
  validateImageFile(file)
  const img = await loadImage(file)
  if (!needsReencode(file.size, img.naturalWidth, img.naturalHeight)) {
    return toDataUrl(file)
  }
  const scale = MAX_LONG_EDGE / Math.max(img.naturalWidth, img.naturalHeight)
  const canvas = document.createElement('canvas')
  canvas.width = Math.max(1, Math.round(img.naturalWidth * scale))
  canvas.height = Math.max(1, Math.round(img.naturalHeight * scale))
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new ImageError('CompressionFailed', 'Canvas 2D context unavailable')
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
  return canvas.toDataURL('image/jpeg', JPEG_QUALITY)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd frontend && npm run test:run -- image.test.ts && npm run typecheck`
Expected: PASS; typecheck clean.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/image.ts frontend/tests/unit/image.test.ts
git commit -m "feat(frontend): image validation and client-side compression helpers

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: InputArea — paste / pick / drag-drop, chips, submit with image parts

**Files:**
- Modify: `frontend/src/types/index.ts:20-21` (the `UserInput` variant)
- Modify: `frontend/src/components/inputs/InputArea.tsx`

**Interfaces:**
- Consumes: `compressImageFile`, `ImageError`, `MAX_IMAGES_PER_MESSAGE` from Task 6; `ConversationEntry` `UserInput.images` (added in this task, used by Task 8).
- Produces: `agent.submit` payload with `input.parts = [{type:'text',...}, ...{type:'image_url', url: dataUrl}]`; optimistic `UserInput` entries carry `images: string[]`.

- [ ] **Step 1: Extend the UserInput type**

In `frontend/src/types/index.ts`, change the `UserInput` variant to:

```ts
  | { type: 'UserInput'; text: string; images?: string[] }
```

- [ ] **Step 2: Add attachment state + handlers to InputArea.tsx**

Import the Task 6 helpers and `PaperclipIcon` + `XIcon` from `lucide-react`:

```ts
import { compressImageFile, ImageError, MAX_IMAGES_PER_MESSAGE } from '@/lib/image'
import { PaperclipIcon, XIcon } from 'lucide-react'
```

Add the attachment state type and state (next to the existing `useState` calls):

```ts
interface ImageAttachment {
  id: string
  dataUrl: string | null // null while compressing
  error: string | null
}
```

```ts
  const [images, setImages] = useState<ImageAttachment[]>([])
  const fileInputRef = useRef<HTMLInputElement>(null)
```

Add the `addFiles` callback (after `submit`):

```ts
  const addFiles = useCallback(
    (files: File[]) => {
      const imageFiles = files.filter((f) => f.type.startsWith('image/'))
      if (imageFiles.length === 0) return
      const room = MAX_IMAGES_PER_MESSAGE - images.length
      if (room <= 0) return
      const selected = imageFiles.slice(0, room)
      const pending: ImageAttachment[] = selected.map((f, i) => ({
        id: `${Date.now()}-${i}-${f.name}`,
        dataUrl: null,
        error: null,
      }))
      // State updaters stay pure (no side effects inside setState — React
      // StrictMode double-invokes updaters); compression runs outside them.
      setImages((prev) => [...prev, ...pending])
      selected.forEach((f, i) => {
        void compressImageFile(f).then(
          (dataUrl) => {
            setImages((cur) => cur.map((a) => (a.id === pending[i].id ? { ...a, dataUrl } : a)))
          },
          (err: unknown) => {
            const message = err instanceof ImageError ? err.message : 'Could not process the image'
            setImages((cur) => cur.map((a) => (a.id === pending[i].id ? { ...a, error: message } : a)))
          },
        )
      })
    },
    [images],
  )

  const removeImage = useCallback((id: string) => {
    setImages((prev) => prev.filter((a) => a.id !== id))
  }, [])
```

- [ ] **Step 3: Wire paste, picker, and drop**

Add to the textarea element in the JSX: `onPaste={handlePaste}`. Define above the return:

```ts
  const handlePaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const files = Array.from(e.clipboardData?.files ?? [])
      if (files.length > 0) {
        e.preventDefault()
        addFiles(files)
      }
    },
    [addFiles],
  )

  const handleDrop = useCallback(
    (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault()
      addFiles(Array.from(e.dataTransfer.files))
    },
    [addFiles],
  )
```

Add `onDragOver={(e) => e.preventDefault()}` and `onDrop={handleDrop}` to the root `<div>` of the returned JSX. Add the hidden file input plus an attach button inside the bottom bar `<div>` (before the "New Session" button):

```tsx
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          multiple
          className="hidden"
          onChange={(e) => {
            addFiles(Array.from(e.target.files ?? []))
            e.target.value = ''
          }}
        />
        <Button
          variant="ghost"
          size="sm"
          className="cursor-pointer text-muted-foreground/60 hover:text-yellow-400/70"
          onClick={() => fileInputRef.current?.click()}
          disabled={isRunning}
          aria-label="Attach images"
        >
          <PaperclipIcon data-icon="inline-start" />
          Attach
        </Button>
```

- [ ] **Step 4: Render the chips row**

Between the `<textarea>` and the bottom bar `<div>`, render the chips row (only when `images.length > 0`):

```tsx
      {images.length > 0 && (
        <div className="flex flex-wrap gap-2 mt-2">
          {images.map((img) => (
            <div key={img.id} className="relative">
              {img.error ? (
                <div className="text-destructive text-xs border border-destructive/50 rounded-md px-2 py-1">
                  {img.error}
                  <button
                    type="button"
                    onClick={() => removeImage(img.id)}
                    className="ml-1 cursor-pointer"
                    aria-label="Remove failed image"
                  >
                    <XIcon data-icon="inline-start" />
                  </button>
                </div>
              ) : img.dataUrl ? (
                <>
                  <img
                    src={img.dataUrl}
                    alt="attachment"
                    className="w-16 h-16 object-cover rounded-md border border-border"
                  />
                  <button
                    type="button"
                    onClick={() => removeImage(img.id)}
                    className="absolute -top-1 -right-1 bg-background border border-border rounded-full cursor-pointer"
                    aria-label="Remove image"
                  >
                    <XIcon data-icon="inline-start" />
                  </button>
                </>
              ) : (
                <div className="w-16 h-16 flex items-center justify-center border border-border rounded-md text-muted-foreground text-xs">
                  ...
                </div>
              )}
            </div>
          ))}
        </div>
      )}
```

- [ ] **Step 5: Update submit to send image parts**

Replace the `submit` callback's guard and payload:

```ts
  const submit = useCallback(() => {
    const input = text.trim()
    const readyImages = images
      .filter((img) => img.dataUrl && !img.error)
      .map((img) => img.dataUrl as string)
    if ((!input && readyImages.length === 0) || isRunning || !selectedAgentId) return
    if (images.some((img) => !img.dataUrl && !img.error)) return // still compressing
```

and the optimistic entry:

```ts
    const userEntry = {
      type: 'UserInput' as const,
      text: input,
      images: readyImages.length > 0 ? readyImages : undefined,
    }
```

and the payload parts:

```ts
        input: {
          parts: [
            ...(input ? [{ type: 'text', text: input }] : []),
            ...readyImages.map((url) => ({ type: 'image_url', url })),
          ],
          metadata: { session_id: sessionId },
        },
```

After a successful `agent.submit` response (in the `.then()` — add one if none exists), clear attachments with `setImages([])`. On error (existing `.catch`), also restore images: `setImages(images)`. Add `images` to the `useCallback` dependency array of `submit`.

- [ ] **Step 6: Verify with typecheck, unit tests, and lint**

Run: `cd frontend && npm run typecheck && npm run test:run && npm run lint`
Expected: all clean (existing `input-area.test.ts` still passes — `findRunIdForAgent` untouched).

- [ ] **Step 7: Commit**

```bash
git add frontend/src/types/index.ts frontend/src/components/inputs/InputArea.tsx
git commit -m "feat(frontend): attach/paste images in the chat input with preview chips

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: Render thumbnails + session-conversion image extraction

**Files:**
- Modify: `frontend/src/components/panels/ConversationView.tsx:110-112` (UserInput rendering)
- Modify: `frontend/src/lib/session-conversion.ts` (`messageText` → `extractParts`, user branch)
- Test: `frontend/tests/unit/session-conversion.test.ts`

**Interfaces:**
- Consumes: `UserInput.images` (Task 7); wire multipart shape `{type:"image", image_url:{url}}` (serde `ContentPart` with `tag="type", rename_all="lowercase"` → tag value `"image"`, field `image_url`).
- Produces: `sessionEntriesToConversation` emits `UserInput` entries with `images: string[]`.

- [ ] **Step 1: Write the failing tests**

Create `frontend/tests/unit/session-conversion.test.ts`:

```ts
// frontend/tests/unit/session-conversion.test.ts
import { describe, it, expect } from 'vitest'
import { sessionEntriesToConversation } from '@/lib/session-conversion'

function userMessageEntry(content: unknown): unknown {
  return {
    type: 'message',
    created_at: 1,
    data: { message: { message: { message: { role: 'user', content } } } },
  }
}

describe('sessionEntriesToConversation — image parts', () => {
  it('extracts image URLs from multipart user content', () => {
    const entries = [
      userMessageEntry([
        { type: 'text', text: 'look at this' },
        { type: 'image', image_url: { url: 'data:image/png;base64,QUJD' } },
      ]),
    ] as never
    const conv = sessionEntriesToConversation(entries)
    expect(conv).toHaveLength(1)
    expect(conv[0].type).toBe('UserInput')
    if (conv[0].type === 'UserInput') {
      expect(conv[0].text).toBe('look at this')
      expect(conv[0].images).toEqual(['data:image/png;base64,QUJD'])
    }
  })

  it('image-only multipart yields empty text with images', () => {
    const entries = [
      userMessageEntry([{ type: 'image', image_url: { url: 'https://e.test/a.png' } }]),
    ] as never
    const conv = sessionEntriesToConversation(entries)
    if (conv[0].type === 'UserInput') {
      expect(conv[0].text).toBe('')
      expect(conv[0].images).toEqual(['https://e.test/a.png'])
    }
  })

  it('text-only content has no images field', () => {
    const entries = [userMessageEntry('plain text')] as never
    const conv = sessionEntriesToConversation(entries)
    if (conv[0].type === 'UserInput') {
      expect(conv[0].text).toBe('plain text')
      expect(conv[0].images).toBeUndefined()
    }
  })
})
```

The fixtures are loose on purpose; the extraction only reads `content`, so cast the entry arrays as `unknown as SessionEntry[]` at the call site (import `SessionEntry` from `@/lib/protocol`) — the assertions above are the contract, the casts are plumbing.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd frontend && npm run test:run -- session-conversion.test.ts`
Expected: FAIL — `UserInput` entries today render the image part as the literal text `"image"`.

- [ ] **Step 3: Implement extraction in session-conversion.ts**

Replace the `messageText` function (lines 33-49) with:

```ts
/** Split wire content into display text and image data URLs. */
function extractParts(content: unknown): { text: string; images: string[] } {
  if (typeof content === 'string') return { text: content, images: [] }
  if (Array.isArray(content)) {
    const text: string[] = []
    const images: string[] = []
    for (const part of content) {
      if (part && typeof part === 'object') {
        const rec = part as Record<string, unknown>
        if (typeof rec.text === 'string') {
          text.push(rec.text)
        } else if (rec.type === 'image') {
          const img = (rec.image_url ?? {}) as Record<string, unknown>
          if (typeof img.url === 'string') images.push(img.url)
        } else if (typeof rec.type === 'string') {
          text.push(rec.type) // unknown future part types degrade to their name
        }
      }
    }
    return { text: text.filter(Boolean).join('\n'), images }
  }
  return { text: '', images: [] }
}
```

Update the `role === 'user'` branch (line 61-62) to:

```ts
        if (role === 'user') {
          const { text, images } = extractParts(msg.content)
          out.push({
            type: 'UserInput',
            text,
            images: images.length > 0 ? images : undefined,
          })
        } else if (role === 'assistant') {
```

and the assistant branch's `messageText(msg.content)` call to `extractParts(msg.content).text`. Also update the doc comment at the top of the file: mention that multipart user content renders image parts as `images`.

- [ ] **Step 4: Render thumbnails in ConversationView.tsx**

Replace the UserInput content block (lines 110-112):

```tsx
        {entry.type === 'UserInput' && (
          <div className="text-foreground whitespace-pre-wrap">{entry.text}</div>
        )}
```

with:

```tsx
        {entry.type === 'UserInput' && (
          <div>
            <div className="text-foreground whitespace-pre-wrap">{entry.text}</div>
            {entry.images && entry.images.length > 0 && (
              <div className="flex flex-wrap gap-2 mt-2">
                {entry.images.map((src, i) => (
                  <img
                    key={i}
                    src={src}
                    alt={`attachment ${i + 1}`}
                    className="w-24 h-24 object-cover rounded-md border border-border"
                  />
                ))}
              </div>
            )}
          </div>
        )}
```

- [ ] **Step 5: Run tests, typecheck, lint**

Run: `cd frontend && npm run test:run && npm run typecheck && npm run lint`
Expected: all clean.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/lib/session-conversion.ts frontend/src/components/panels/ConversationView.tsx frontend/tests/unit/session-conversion.test.ts
git commit -m "feat(frontend): render attached-image thumbnails in conversation history

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 9: WS frame-size verification, end-to-end check, gates, docs

**Files:**
- Modify: only if a WS message-size limit below ~2MB is found (see Step 1).
- No test files unless a config change is required (then add a config-validation test per the crate's existing pattern).

**Interfaces:**
- Consumes: everything from Tasks 1-8.
- Produces: verified end-to-end feature + green gates.

- [ ] **Step 1: Verify WS frame size limits**

Run: `rg -n "max_message_size|max_frame_size|message_size" crates/vol-llm-agent-protocol crates/vol-agent-server`
Expected: document what you find. If any limit is set below `2_097_152` (2MB), raise it to at least 2MB (4 images × ≤300KB × 4/3 base64 ≈ 1.6MB) with a config-validation or unit test following the crate's existing test patterns, and commit that change separately:
`git commit -m "fix(protocol): raise WS message size limit for image payloads"`.
If no limit is configured (axum/tungstenite defaults), record "no explicit limit" in the commit body of Step 4 and move on.

- [ ] **Step 2: Backend test + gate sweep**

Run:
```bash
cargo test -p vol-llm-core -p vol-llm-agent -p vol-llm-context -p vol-session -p vol-llm-provider
just cover-gate vol-llm-core 80
just cover-gate vol-llm-agent 80
just cover-gate vol-llm-context 80
just cover-gate vol-session 80
just cover-gate vol-llm-provider 80
./scripts/check-no-doc-tests.sh
```
Expected: all tests pass; every crate ≥ 80% (exception crates `main.rs`, `app.rs`, `health.rs` exempt).

- [ ] **Step 3: Manual end-to-end verification**

Run the stack: `just web-backend` (terminal 1) and `just web-dev` (terminal 2). In the browser at `localhost:5173`:
1. Select an agent using the local qwen3.6-plus provider; paste an image (Ctrl+V) with a question — expect the agent to describe the image correctly.
2. Attach via the paperclip button; drag-drop a third image; verify chips, remove-button, and thumbnails in the conversation.
3. Start a new session, attach an image, run — then reload the page and re-open the session; expect the thumbnail in history and the agent able to answer follow-ups about the image (image re-sent from session history).
4. Check the Context tab: the session's estimated token count should be in the low thousands for an image conversation, not hundreds of thousands.
5. Verify text-only messages behave exactly as before.

- [ ] **Step 4: Ingest results into the wiki and commit**

Run the wiki-ingest skill for the implementation (per CLAUDE.md: task done → `wiki-ingest`), then:

```bash
git add docs/wiki
git commit -m "docs(wiki): ingest multimodal image input implementation

Co-Authored-By: Claude <noreply@anthropic.com>"
```

- [ ] **Step 5: Upload spec/plan docs to Lark**

Per CLAUDE.md superpowers mapping: upload `docs/superpowers/plans/2026-08-17-multimodal-image-input.md` to Lark node `TEkkw1W6niuBxQkcvswchOo5nhb` and `docs/superpowers/specs/2026-08-17-multimodal-image-input-design.md` to node `Og7twpiPoi0Vbjk2EzvcqX92nsb` using the `lark-cli docs` commands from CLAUDE.md.

---

## Self-Review Notes

- Spec coverage: §1 data flow → Tasks 6/7 (browser→wire) + verified existing path; §2 compression policy → Task 6 constants; §3 table → Tasks 1-8 (openai_streaming verified no-op); §4 session semantics → Task 4; §5 error handling → Task 6 (`ImageError`) + Task 7 (chips/submit guard); §6 testing → per-task tests + Task 9 sweep; §7 success criteria → Task 9 Step 3.
- Type consistency: `display_text` signatures match across Tasks 1/2/4; `IMAGE_TOKEN_BUDGET` name used consistently in Task 3 tests and implementation; frontend exports in Task 6 match Task 7 imports; wire tag `"image"` (serde lowercase) matches Task 8 extraction and tests.
- No placeholders: all code steps contain full code; the one "document what you find" step (Task 9 Step 1) is a verification step with explicit commands and decision branches.
