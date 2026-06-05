---
title: Rich Text Conversation Rendering
status: active
updated: 2026-06-04
related: [vol-llm-ui-crate, conversation-view]
---

# Rich Text Conversation Rendering

Renders markdown in agent answers, streaming output, and tool results on the web frontend. Implemented as a Rust/JS handoff: Dioxus emits sentinel containers, a JS module renders them via marked + DOMPurify + highlight.js.

## Why this design

The original conversation view (`crates/vol-llm-ui/src/web/components/conversation.rs`) rendered all text as plain `<div>` with `whitespace-pre-wrap`. LLM-produced markdown stayed literal. Adding a Rust markdown crate (pulldown-cmark or comrak) would have:
- Increased WASM bundle size by 200-500KB
- Required HTML sanitization on the Rust side
- Coupled rendering choice to Rust compile time

Instead, the Rust side is intentionally markdown-unaware: it emits a `<div data-md="1">` containing `<pre data-md-raw>原文</pre>`. The browser-side `markdown.js` (embedded via `include_str!()` + `dioxus::document::eval()`) watches for these via MutationObserver, debounces high-frequency mutations (streaming output), and renders the result.

## Data flow

```
Dioxus AgentAnswer/ContentStreaming/ToolResult
   ↓ rsx! { div data-md=1 dangerous_inner_html=<pre data-md-raw>… }
Browser DOM
   ↓ MutationObserver (childList | characterData | subtree)
markdown.js queue
   ↓ debounce 100ms
   ↓ requestIdleCallback
render(node):
   marked.parse(raw) → DOMPurify.sanitize(html) → node.innerHTML = safe
   hljs.highlightElement(each pre code)
   maybeStickToBottom() — preserves user's scroll position
```

## Key files

| Path | Role |
|------|------|
| `crates/vol-llm-ui/assets/markdown.js` | Render pipeline; MutationObserver; CDN detection; scroll integration |
| `crates/vol-llm-ui/assets/markdown.css` | Dark-theme markdown styles; fallback styles for CDN failure |
| `crates/vol-llm-ui/src/web/components/app.rs` | `include_str!()` embeds markdown.js; `document::Stylesheet` for markdown.css |
| `crates/vol-llm-ui/src/web/components/conversation.rs` | `html_escape()` and `markdown_container()` helpers; applied at 4 render sites |
| `crates/vol-llm-ui/index.html` | CDN script tags (marked, DOMPurify, highlight.js) loaded synchronously |

## Sites that render as markdown

- `AgentAnswer` — final agent response
- `ContentStreaming` — live streaming agent response
- `ToolResult` preview — 2-line summary on the tool card
- `ToolDetailModal` result body — full tool result inside the modal

## Sites that stay plain text (deliberate)

- `UserInput` — user's own message
- `Thinking` — agent's chain-of-thought (italic gray)
- `ToolCall` argument preview — typically short JSON, no markdown to render
- `ToolDetailModal` arguments — JSON, kept as raw text for readability
- All TUI rendering

## Failure modes

| Failure | Behavior |
|---------|----------|
| CDN libs fail to load (5s timeout) | `body.markdown-fallback` class applied; `<pre data-md-raw>` styled as plain text |
| `marked.parse` or `DOMPurify.sanitize` throws | Node gets `markdown-error` class; falls back to plain `<pre>` style |
| Individual `hljs.highlightElement` throws | That block stays unhighlighted; rest of node renders |
| Single message > 50KB | Skipped; raw text fallback |

## Performance budget (verified)

| Metric | Budget |
|--------|--------|
| CDN payload | < 250KB gzipped |
| Streaming render frequency | ≤ 10/sec via 100ms debounce |
| Per-render cost | < 16ms via `requestIdleCallback` |

## See also

- Design: `docs/superpowers/specs/2026-06-04-rich-text-conversation-design.md`
- Plan: `docs/superpowers/plans/2026-06-04-rich-text-conversation.md`
- Scroll mechanism: [[conversation-view]] (`data-auto-scroll`, `data-scroll-programmatic`)
