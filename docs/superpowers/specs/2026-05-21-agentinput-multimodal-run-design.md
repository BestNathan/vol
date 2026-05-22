# AgentInput Multimodal Run Design

## Goal

Extend `ReActAgent` run input from plain text to a structured input envelope that supports multimodal parts and run-level parameters, while preserving the existing `run(&str)` convenience API.

The first implementation supports text plus image URL/data URL parts. It deliberately does not read local files, infer MIME types, download remote content, or validate image payloads in the agent layer.

## Public API

Add an `AgentInput` type for one run request:

```rust
pub struct AgentInput {
    pub run_id: Option<String>,
    pub parts: Vec<InputPart>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

Add `InputPart`:

```rust
pub enum InputPart {
    Text { text: String },
    ImageUrl { url: String, detail: Option<String> },
}
```

Keep the existing text API and add a structured API:

```rust
pub async fn run(&self, user_input: &str) -> Result<AgentResponse, AgentError>
pub async fn run_input(&self, input: AgentInput) -> Result<AgentResponse, AgentError>
```

`run(&str)` is a convenience wrapper over `run_input(AgentInput::text(user_input))`. Existing call sites and tests should continue to compile without changes unless they intentionally exercise multimodal input.

`AgentInput` should provide small builders for ergonomic construction:

```rust
impl AgentInput {
    pub fn text(text: impl Into<String>) -> Self;
    pub fn new() -> Self;
    pub fn with_run_id(self, run_id: impl Into<String>) -> Self;
    pub fn with_metadata_value(self, key: impl Into<String>, value: serde_json::Value) -> Self;
    pub fn text_part(self, text: impl Into<String>) -> Self;
    pub fn image_url(self, url: impl Into<String>) -> Self;
    pub fn image_url_with_detail(self, url: impl Into<String>, detail: impl Into<String>) -> Self;
}
```

## Agent Data Flow

`run_input` converts `AgentInput.parts` into `vol_llm_core::MessageContent` before persisting the user message:

- A single text part becomes `MessageContent::Text`.
- Multiple parts or any image part become `MessageContent::MultiPart(Vec<ContentPart>)`.
- Text maps to `ContentPart::Text { text }`.
- Image URL/data URL maps to `ContentPart::Image { image_url: ImageUrl { url, detail } }`.

`run_input` uses `AgentInput.run_id` when present. If absent, it keeps the current behavior and generates a UUID. The chosen run ID must be used consistently in `RunContext`, emitted events, and `AgentResponse`.

`metadata` is run-level metadata. The first implementation should keep it on `AgentInput` and pass it through the run context/event boundary only where an existing metadata slot is available. It should not force a session schema change unless implementation discovers an existing safe metadata path.

## Provider Conversion

`vol_llm_core::MessageContent` already supports multipart content, but the Anthropic provider currently reads user content through `as_str()`, which loses multipart data. Update Anthropic user-message conversion to emit content blocks:

- `MessageContent::Text(text)` may continue to serialize as a plain string or as a single text block.
- `MessageContent::MultiPart(parts)` serializes to an Anthropic content array.
- Text parts serialize as `{ "type": "text", "text": ... }`.
- Image URL parts serialize as `{ "type": "image", "source": { "type": "url", "url": ... } }`.
- Image data URLs should be parsed into media type and base64 data, then serialized as `{ "type": "image", "source": { "type": "base64", "media_type": ..., "data": ... } }`.
- Invalid data URL syntax returns a provider conversion error before making the HTTP request.

Assistant and tool messages should keep their existing behavior unless needed for type compatibility.

## Channel Compatibility

`vol-llm-agent-channel` currently accepts `input: String` and calls `ReActAgent::run(&input)`. Extend it so structured requests route to `run_input` without breaking old clients.

Preferred protocol shape:

```rust
pub input: AgentInput
```

Serde compatibility should allow both old and new JSON:

```json
{ "input": "hello" }
```

and

```json
{
  "input": {
    "run_id": "optional-run-id",
    "parts": [
      { "type": "text", "text": "look at this" },
      { "type": "image_url", "url": "data:image/png;base64,..." }
    ],
    "metadata": {}
  }
}
```

The Rust convenience constructors `AgentRequest::new(target_id, input)` and `AgentRequest::with_id(req_id, target_id, input)` should continue to accept plain text. Add explicit structured constructors when needed.

## Validation and Errors

Return an `AgentError` before starting the run when `parts` is empty.

The agent layer does not fetch URLs, decode base64, inspect MIME types, or enforce image dimensions. Provider/API errors remain provider errors.

`ImageUrl.detail` is passed through as an optional string in the first implementation. A future change may constrain it to a provider-neutral enum if needed.

## Extensibility

Future modalities should extend `InputPart`, not change the shape of `AgentInput`:

```rust
AudioUrl { url: String, mime_type: Option<String> }
AudioBase64 { data: String, mime_type: String }
```

Provider support can then be added incrementally per provider without changing agent call sites.

## Tests

Add or update tests for:

1. `run("text")` still persists and sends a text user message.
2. `run_input(AgentInput::text("text"))` behaves like `run("text")`.
3. Text plus image input persists as `MessageContent::MultiPart`.
4. A caller-provided `run_id` appears in `AgentResponse.run_id` and run events.
5. Empty `parts` returns an error before any LLM call.
6. Anthropic conversion preserves multipart text and image blocks.
7. Channel protocol accepts old string input and new structured input.
