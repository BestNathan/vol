---
type: concept
category: pattern
tags: [session, compression, summarization]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Session Compression

**Category:** Token management pattern
**Related:** [[session-as-ssot]], [[session-contributor]], [[context-builder]]

## Definition

Two-layer compression for session message history that reduces token usage while preserving semantic meaning. Layer 1: rule-based tool call summarization. Layer 2: LLM-driven conversation summarization.

## Key Points
- `ToolCallCompressor`: Rule-based, extracts tool name/args/result (truncated) into summary lines [[session-compression]]
- `ConversationCompressor`: LLM-driven, sends dialogue to LLM with summary prompt, returns single user message [[session-compression]]
- `SessionCompressor`: Orchestrator that splits messages, delegates to sub-compressors, merges results [[session-compression]]
- Keeps last 5 messages untouched (configurable via `KEEP_LAST`) [[session-compression]]
- Graceful degradation: if LLM compression fails, includes uncompressed history as fallback [[session-compression]]
- Output format: `[tool_summary_msg (system), conv_summary_msg (user)] + recent messages` [[session-compression]]

## How It Works

```
SessionCompressor
├── split(messages, keep_last: 5)
│   ├── history → separate tool vs conversation messages
│   └── recent  → passthrough (last 5 messages)
├── ToolCallCompressor.compress(tool_messages)
│   └── Returns: 1x system message summarizing all tool calls
├── ConversationCompressor.compress(user_assistant_messages, llm)
│   └── Returns: 1x user message summarizing key decisions/code/issues
└── Merge: [tool_summary?, conv_summary?] + recent
```

### ToolCallCompressor

Rule-based compression. For each tool message, extracts:
- Tool name
- Arguments (truncated to 200 chars)
- Result (truncated to 500 chars)

Produces one summary line per tool: `[tool_name] args → result`

### ConversationCompressor

LLM-driven compression. Sends user/assistant messages to LLM with prompt:
> "Summarize the following conversation. Focus on: 1. Key decisions made 2. Code changes proposed or implemented 3. Open issues or unresolved questions"

Returns a single user message with prefix `[Session Summary]: {summary}`.

### SessionCompressor Orchestrator

1. If `messages.len() <= keep_last`, returns as-is
2. Splits: `history = messages[0..len-keep_last]`, `recent = messages[len-keep_last..]`
3. Partitions history into `tool_msgs` and `conv_msgs`
4. Compresses tool messages → system message (if any)
5. Compresses conversation messages → user message via LLM (if any)
6. Merges: `[tool_summary?, conv_summary?] + recent`

On LLM failure, the conversation summary is skipped and uncompressed history is included as fallback.

## Examples / Applications

- **Long coding sessions**: Compress tool results and intermediate dialogue, keep recent context
- **Token budget management**: Trigger compression when session approaches token limit
- **Graceful degradation**: LLM unavailable → includes full history (no data loss)

## Related Concepts
- [[session-as-ssot]]: Compresses Session messages in place
- [[session-contributor]]: SessionContributor.compress() triggers this
- [[context-builder]]: Compressed messages reduce token usage in context build
