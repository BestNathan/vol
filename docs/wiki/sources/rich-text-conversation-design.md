---
type: source
source_type: design
date: 2026-06-04
ingested: 2026-06-04
tags: [rich-text, markdown, vol-llm-ui, dioxus, conversation]
---

# Rich Text Conversation Design

**Authors/Creators:** Claude Opus 4.8
**Date:** 2026-06-04
**Link:** docs/superpowers/specs/2026-06-04-rich-text-conversation-design.md

## TL;DR
Design for rendering markdown (headings, code blocks with syntax highlighting, tables, inline code) in the vol-llm-ui web frontend. Architecture: Rust/Dioxus emits `<div data-md>` sentinel containers; a JS module (embedded via `include_str!()`) watches via MutationObserver, debounces to 100ms, runs marked + DOMPurify + highlight.js. CDN-loaded with plain-text fallback.

## Key Takeaways
- Markdown rendering via JS (marked + DOMPurify + highlight.js) avoids WASM bundle bloat
- Rust side stays markdown-unaware: only emits `<div data-md="1"><pre data-md-raw>...</pre></div>`
- MutationObserver + 100ms debounce + requestIdleCallback for streaming throttling
- DOMPurify with explicit ALLOWED_TAGS whitelist for XSS defense
- 10 pre-registered highlight.js languages, others unhighlighted but readable
- Every phase independently revertible

## Detailed Summary
The feature was implemented across 4 phases:
1. JS/CSS infrastructure (`markdown.js`, `markdown.css`, CDN script injection)
2. Rust integration (`html_escape()`, `markdown_container()` helper, applied to 4 render sites)
3. Scroll integration (`maybeStickToBottom()` in markdown.js)
4. Tests (Playwright 4-scenario spec) and wiki documentation

Key architectural decision: embed `markdown.js` via `include_str!()` + `dioxus::document::eval()` because Dioxus 0.6 only serves hashed asset URLs, not static paths. CDN scripts (marked, DOMPurify, highlight.js) loaded synchronously in `index.html`.

## Entities Mentioned
- [[vol-llm-ui-crate]]: web frontend crate where all changes live

## Concepts Covered
- [[rich-text-conversation]]: the resulting markdown rendering system
- [[conversation-view]]: scroll mechanism integration point

## Notes
- Playwright CLI tests fail due to localhost access issues; verified via MCP Playwright
- highlight.js atom-one-dark theme loaded from CDN
- lark-cli not installed; feishu wiki upload skipped
