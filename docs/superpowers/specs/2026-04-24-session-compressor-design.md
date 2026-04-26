# Session Compressor Design

> Status: approved — 2026-04-24

## Problem

Session message history grows without bound. When sessions reach token budget limits, the only compression mechanism is dropping blocks — losing information rather than summarizing it. We need LLM-driven compression that preserves semantic meaning.

## Solution

Add a `SessionCompressor` in `vol-llm-agents/src/coding/` that splits compression into two layers:

| Layer | Mechanism | What it compresses |
|-------|-----------|-------------------|
| ToolCallCompressor | Rule-based (no LLM) | Tool call results — truncate to key info |
| ConversationCompressor | LLM-driven | User/assistant dialogue → single user message |

Output: `[tool_summary_msg, conv_summary_msg] + last 5 original messages`

## Architecture

```
SessionCompressor
├── split(messages, keep_last: 5)
│   ├── history → separate tool vs conversation messages
│   └── recent  → passthrough (last 5 messages)
├── ToolCallCompressor.compress(tool_messages)
│   └── Returns: 1x system message summarizing all tool calls
├── ConversationCompressor.compress(user_assistant_messages, llm)
│   └── Returns: 1x user message summarizing key decisions/code/issues
└── Merge: [tool_summary, conv_summary] + recent
```

## Components

### 1. ToolCallCompressor

Rule-based, no LLM needed. Iterates over tool messages:

- Extract tool name, arguments (truncated to 200 chars), result (truncated to 500 chars)
- For each tool call: produce a summary line like `[tool_name] args → result_preview`
- Combine all lines into a single `Message::system`

```rust
pub struct ToolCallCompressor;

impl ToolCallCompressor {
    pub fn compress(&self, tool_messages: &[SessionMessage]) -> SessionMessage;
}
```

### 2. ConversationCompressor

LLM-driven. Collects user + assistant messages, sends them to the LLM with a system prompt requesting a structured summary:

**Prompt structure:**
```
System: "You are a session compressor. Summarize the following conversation into a single paragraph. Focus on:
1. Key decisions made
2. Code changes proposed or implemented
3. Open issues or unresolved questions
Be concise. Output only the summary."

User: "<serialized user/assistant messages>"

Expected output: A single paragraph of prose summarizing the session.
```

Returns a single `Message::user` with the summary text, prefixed with `[Session Summary]: `.

```rust
pub struct ConversationCompressor {
    llm: Arc<dyn LLMClient>,
}

impl ConversationCompressor {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self;
    pub async fn compress(&self, messages: &[SessionMessage]) -> Result<SessionMessage>;
}
```

### 3. SessionCompressor

Orchestrator. Takes all session messages (`Vec<SessionMessage>`), splits at boundary, delegates to sub-compressors.

```rust
pub struct SessionCompressor {
    llm: Arc<dyn LLMClient>,
    keep_last: usize,  // default: 5
}

impl SessionCompressor {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self;
    pub async fn compress(&self, messages: Vec<SessionMessage>) -> Result<Vec<SessionMessage>>;
}
```

**Compression flow:**
1. If `messages.len() <= keep_last`, return as-is (nothing to compress)
2. Split: `history = messages[0..messages.len()-keep_last]`, `recent = messages[messages.len()-keep_last..]`
3. From `history`: partition into `tool_msgs` (role=tool) and `conv_msgs` (role=user or assistant)
4. `tool_summary = ToolCallCompressor.compress(&tool_msgs)` (only if tool_msgs is non-empty)
5. `conv_summary = ConversationCompressor.compress(&conv_msgs).await` (only if conv_msgs is non-empty; uses `self.llm`)
6. Merge summaries + recent: `[tool_summary?, conv_summary?] + recent`

## Error Handling

- LLM compression fails → fallback: include full uncompressed history (degrade gracefully, never lose data)
- Empty tool/conversation history → skip that compressor
- Empty input → return empty vec

## Integration

A `SessionContributor` in `vol-llm-context` (external contributor) will hold a reference to `SessionCompressor`:

```rust
pub struct SessionContributor {
    session: Arc<Session>,
    compressor: Option<SessionCompressor>,
}
```

When `contribute()` is called:
1. If no compressor set → return full session history as-is
2. If compressor set → call `compressor.compress(messages)` and return result as a single `ContextBlock` with `Middle` anchor

## File Structure

| File | Purpose |
|------|---------|
| `crates/vol-llm-agents/src/coding/compressor/tool_call.rs` | ToolCallCompressor |
| `crates/vol-llm-agents/src/coding/compressor/conversation.rs` | ConversationCompressor |
| `crates/vol-llm-agents/src/coding/compressor/mod.rs` | SessionCompressor + re-exports |
| `crates/vol-llm-agents/src/coding/compressor/tests.rs` | Integration tests |

## Compression constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `KEEP_LAST` | 5 | Number of recent messages to preserve untouched |
| `TOOL_ARGS_MAX` | 200 chars | Max chars for tool args in summary |
| `TOOL_RESULT_MAX` | 500 chars | Max chars for tool result in summary |
| `LLM_SUMMARY_MODEL` | inherited | Use the agent's existing LLM |
