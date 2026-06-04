# Rich Text Display in Conversation — Design

**Date:** 2026-06-04
**Status:** Approved (pending implementation plan)
**Scope:** `vol-llm-ui` web frontend only (Dioxus 0.6 WASM)
**Out of scope:** TUI rendering, user-input markdown, lazy language imports, Mermaid / KaTeX, image rendering

---

## 1. Background

The current `vol-llm-ui` web frontend renders all agent output as plain text inside `<div>` elements with `whitespace-pre-wrap`. Dioxus RSX escapes any HTML in the text, so LLM-produced markdown (code blocks, headings, lists, tables, inline code) shows as raw `**bold**` / triple-backtick fences / `# Heading` strings instead of formatted content.

The workspace currently has **no** markdown parser, syntax highlighter, or HTML sanitizer (verified by grep across all Cargo.toml and package.json files). The `assets/input.css` is a minimal Tailwind v4 config with no `@tailwindcss/typography` plugin.

This design adds rich text rendering for agent answers (streaming and final) and tool results, while keeping the Rust side intentionally markdown-unaware.

## 2. Goals

| Goal | Why |
|------|-----|
| Render markdown (headings, lists, bold/italic, links, blockquotes, tables) | Match LLM output expectations |
| Render fenced code blocks with syntax highlighting | Primary developer use case |
| Render inline `` `code` `` with background | Visual distinction in prose |
| Throttle streaming rendering | Avoid DOM thrash at 50+ tokens/sec |
| Keep Rust side ignorant of markdown | Decouples frontend from rendering library choice |
| Degrade gracefully if CDN libs fail to load | Offline / blocked-CDN users still see readable text |
| Defend against XSS in LLM output | LLMs can be prompted to emit `<script>` |

## 3. Non-Goals (YAGNI)

Explicitly **out** of v1 to prevent scope creep:

- Lazy loading of additional highlight.js languages (v1 pre-registers 10 high-frequency languages; others display unhighlighted but readable)
- Mermaid diagram rendering
- KaTeX / MathJax math rendering
- Rendering user input as markdown (`UserInput` stays plain text)
- Rendering `Thinking` content as markdown (stays italic gray)
- TUI markdown support (terminal needs different rendering pipeline)
- Image / video / embed rendering (`<img>` explicitly forbidden by DOMPurify config)
- A markdown editor for user input (textarea stays plain)

## 4. Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Browser (vol-llm-ui WASM)                     │
│                                                                  │
│   Rust / Dioxus layer                Browser / JS layer          │
│   ┌──────────────────┐              ┌──────────────────────┐    │
│   │ TimelineEntry    │              │ markdown.js          │    │
│   │ (AgentAnswer,    │  raw text    │ (own module)         │    │
│   │  ContentStream-  ├─────────────►│                      │    │
│   │  ing, ToolResult)│  via         │ MutationObserver     │    │
│   │                  │  data-md +   │  ↓                   │    │
│   │ <div data-md=… ▶ │  <pre>       │ debounce(100ms)      │    │
│   │   <pre data-md-  │              │  ↓                   │    │
│   │   raw>原文</pre> │              │ requestIdleCallback  │    │
│   │ </div>           │              │  ↓                   │    │
│   └──────────────────┘              │ marked()             │    │
│                                      │  ↓                   │    │
│                                      │ DOMPurify()          │    │
│                                      │  ↓                   │    │
│                                      │ hljs.highlightElement│    │
│                                      │  ↓                   │    │
│                                      │ replace innerHTML    │    │
│                                      └──────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
        ▲                                          ▲
        │                                          │
   Tailwind base                          CDN (jsdelivr)
   + markdown.css                         marked + highlight.js + DOMPurify
   (styles applied to                     + highlight.js theme CSS
    rendered HTML)
```

**Key design point:** The Rust side never imports a markdown crate. It only emits a sentinel container that JS picks up via MutationObserver. This isolates frontend rendering technology from Rust compilation time and WASM bundle size.

## 5. Components

### 5.1 New files

| File | Lines | Purpose |
|------|-------|---------|
| `crates/vol-llm-ui/assets/markdown.js` | ~120 | CDN load detection, MutationObserver, debounce queue, `render()` function, scroll integration, error isolation |
| `crates/vol-llm-ui/assets/markdown.css` | ~80 | Dark-theme styles for rendered markdown elements; fallback styles |

### 5.2 Modified files

| File | Lines changed | Change |
|------|---------------|--------|
| `crates/vol-llm-ui/src/web/components/app.rs` | +3 | Declare `markdown.css` and `markdown.js` as assets; emit `<link>` and `<script>` tags |
| `crates/vol-llm-ui/Dioxus.toml` or `index.html` template | +6 | Inject CDN `<script>` tags + highlight.js theme `<link>` |
| `crates/vol-llm-ui/src/web/components/conversation.rs` | +20, -6 | Add `markdown_container()` helper; apply to `AgentAnswer`, `ContentStreaming`, `ToolResult` preview, and `ToolDetailModal` result body |

### 5.3 `markdown_container()` helper

In `conversation.rs`, a single helper function that any rich-text rendering site calls:

```rust
fn markdown_container(text: &str) -> Element {
    let raw = format!("<pre data-md-raw>{}</pre>", html_escape(text));
    rsx! {
        div {
            class: "text-[#e0e0e0] leading-[1.5]",
            "data-md": "1",
            dangerous_inner_html: raw
        }
    }
}
```

Where `html_escape()` replaces `&`, `<`, `>`, `"` with their entity equivalents — required because `dangerous_inner_html` does not escape the wrapped `<pre>` content. The `<pre>` ensures whitespace preservation and prevents the browser from parsing the raw markdown as HTML on first paint (fallback behavior).

### 5.4 `markdown.js` responsibilities

The module's contract:

1. **Startup**: poll for `window.marked`, `window.DOMPurify`, `window.hljs` to be defined (max 5s); on failure, add `markdown-fallback` class to `<body>` and exit.
2. **Pre-register high-frequency languages**: `javascript`, `typescript`, `python`, `rust`, `bash`, `json`, `yaml`, `html`, `css`, `sql`. Other languages render as un-highlighted code (still readable, monospace).
3. **MutationObserver** on `document.body` with `{ childList: true, subtree: true, characterData: true }`. Filter mutations to those affecting nodes with the `[data-md]` attribute.
4. **Debounce queue**: when a `[data-md]` node mutates, add to queue; if no flush pending, `setTimeout(flush, 100)`.
5. **`flush()`**: inside `requestIdleCallback`, call `render(node)` for each queued node, then clear the queue.
6. **`render(node)`**:
   - Read raw markdown from `node.querySelector('pre[data-md-raw]')?.textContent` (or fall back to `node.textContent` if structure missing)
   - Compute simple hash of raw text; if `node.dataset.mdRendered === hash`, skip
   - `let html = marked.parse(raw)`
   - `let safe = DOMPurify.sanitize(html, SANITIZE_CONFIG)`
   - `node.innerHTML = safe`
   - `node.dataset.mdRendered = hash`
   - For each `pre code` in node: `hljs.highlightElement(codeEl)`
   - If `document.querySelector('[data-scroll][data-auto-scroll="1"]')` exists, set `data-scroll-programmatic="1"` and scroll to bottom
   - Wrap entire body in `try / catch` → on error, console.error and add `markdown-error` class to node (which falls back to `pre`-style display)

### 5.5 DOMPurify configuration

```js
const SANITIZE_CONFIG = {
  ALLOWED_TAGS: ['p','br','strong','em','del','code','pre','blockquote',
                 'ul','ol','li','h1','h2','h3','h4','h5','h6',
                 'a','table','thead','tbody','tr','th','td','hr'],
  FORBID_TAGS: ['script','iframe','object','embed','img','video','audio','style','link'],
  ADD_ATTR: ['target','rel'],
  ALLOWED_ATTR: ['href','target','rel','class']  // class for hljs syntax classes
};

// afterSanitizeAttributes hook: force all <a> to target=_blank rel=noopener noreferrer
DOMPurify.addHook('afterSanitizeAttributes', (node) => {
  if (node.tagName === 'A') {
    node.setAttribute('target', '_blank');
    node.setAttribute('rel', 'noopener noreferrer');
  }
});
```

## 6. Data flow

### 6.1 Static message path

1. User scrolls into view OR new message arrives
2. Dioxus renders `<div data-md="1" dangerous_inner_html='<pre data-md-raw>原文</pre>'>`
3. MutationObserver fires (node added)
4. `queue.add(node)`; if no flush pending, schedule `setTimeout(flush, 100)`
5. After debounce window, `requestIdleCallback` calls `render()` for each queued node

### 6.2 Streaming message path

1. `ContentStart` event → Dioxus renders `ContentStreaming` variant → `markdown_container("")` emits `<div data-md="1"><pre data-md-raw></pre></div>`
2. Each `ContentDelta` → Dioxus replaces the `dangerous_inner_html` value → `<pre>` text content changes
3. MutationObserver fires on `characterData`; `queue.add(node)`
4. Debounce coalesces N rapid mutations to 1 render per 100ms
5. `render()` parses partial markdown; marked tolerates unclosed fences (renders trailing content as code block)
6. `ContentComplete` → Dioxus replaces the variant with `AgentAnswer { text }` → entire node subtree replaced
7. MutationObserver fires; final `render()` renders the complete markdown

### 6.3 Scroll integration

The existing `[data-scroll]` container has a `data-auto-scroll` attribute reflecting whether the user wants stick-to-bottom. After each markdown render, `markdown.js` checks this attribute and, if `"1"`, sets `data-scroll-programmatic="1"` on the scroll container and assigns `scrollTop = scrollHeight`. The `onscroll` handler in `conversation.rs` already skips events with the programmatic flag, so no `auto_scroll` state change is triggered.

## 7. Error handling

| Failure mode | Detection | Behavior |
|--------------|-----------|----------|
| CDN scripts fail to load | 5s timeout polling `window.marked`/`DOMPurify`/`hljs` | Add `markdown-fallback` class to `<body>`; `<pre data-md-raw>` styling gives readable plain text |
| `marked.parse()` throws on malformed input | `try/catch` in `render()` | console.error; add `markdown-error` class; node displays as `<pre>` |
| `DOMPurify.sanitize()` throws | `try/catch` in `render()` | Same as above |
| `hljs.highlightElement()` throws on one code block | per-code-block `try/catch` | Skip highlighting that block; rest of node renders normally |
| Single message > 50KB | length check before parse | Skip markdown rendering; show `<pre>` plain text |

## 8. Security

**XSS defense layers** (defense in depth):

1. Dioxus escapes any text interpolation by default; only `dangerous_inner_html` bypasses, and we control its content
2. Our `html_escape()` wraps user/LLM content before injection so `<` inside `<pre data-md-raw>` becomes `&lt;`
3. DOMPurify with explicit `ALLOWED_TAGS` whitelist filters anything `marked` might pass through
4. `FORBID_TAGS` explicitly bars `<script>`, `<iframe>`, `<object>`, `<embed>`, `<img>`, `<video>`, `<audio>`, `<style>`, `<link>`
5. Hook forces `target="_blank" rel="noopener noreferrer"` on all anchors
6. CSP could further restrict (future enhancement; not required for v1)

## 9. Performance

| Metric | Budget | Strategy |
|--------|--------|----------|
| CDN payload | < 250 KB gzipped | marked ~20KB + DOMPurify ~25KB + highlight.js core ~30KB + 10 languages ~40KB |
| Render frequency under streaming | ≤ 10 / sec | 100ms debounce |
| Per-render cost | < 16ms (one frame) | `requestIdleCallback` defers to browser idle |
| Historical message batch (100 entries) | < 500ms total | Idle-callback natural batching |
| MutationObserver overhead | negligible | One observer on body; V8 optimizes attribute/subtree filtering |

**Highlight.js language strategy**:
- v1 pre-registers 10 common languages totaling ~40KB
- Unknown languages render as plain `<code>` (no highlight, still readable)
- Lazy import for additional languages is explicitly deferred to a future iteration

## 10. Testing

| Layer | Tool | Coverage |
|-------|------|----------|
| Rust unit | `cargo test -p vol-llm-ui --features web` | `markdown_container()` produces correct DOM structure: `data-md="1"` attribute, `<pre data-md-raw>` wrapping, HTML-escaped content |
| JS integration | Playwright | (a) markdown renders correctly; (b) streaming debounce yields ≤10 renders/sec; (c) CDN failure shows fallback styling; (d) `<script>alert(1)</script>` is stripped |
| Visual regression | Manual screenshot archive | Representative messages: headings, lists, code blocks (Python, Rust, JS), tables, long inline code, mixed prose |

## 11. Implementation phases

**Phase 1 — Infrastructure (1 PR)**
- Inject CDN scripts into `index.html` template
- Create `assets/markdown.js` (CDN detection, MutationObserver, debounce, render, 10 language registrations)
- Create `assets/markdown.css` (dark theme + fallback)
- Manual verification: hand-inject a `<div data-md><pre data-md-raw># Hi</pre></div>` and confirm rendering

**Phase 2 — Rust integration (1 PR)**
- Add `markdown_container()` helper in `conversation.rs`
- Replace 4 render sites: `AgentAnswer`, `ContentStreaming`, `ToolResult` preview, `ToolDetailModal` result body
- `cargo check`, `cargo clippy`, `make web-check`

**Phase 3 — Scroll integration (1 PR)**
- Extend `markdown.js` `render()` to call `scrollTop = scrollHeight` when `data-auto-scroll="1"`, with `data-scroll-programmatic` flag
- Verify: streaming output keeps stick-to-bottom; user scroll up is not reverted

**Phase 4 — Tests and docs (1 PR)**
- Playwright test script for the 4 scenarios
- Visual screenshot archive committed under `docs/screenshots/rich-text/`
- `wiki-ingest` skill updates `docs/wiki/concepts/rich-text-conversation.md`

## 12. Rollback strategy

Every phase is independently revertible:

- Phase 1 fails → delete `markdown.js`, `markdown.css`, undo `index.html` injection → no Rust code touched
- Phase 2 fails → change `markdown_container(text)` body to `rsx! { div { class: "...", "{text}" } }` (one-line revert per site)
- Phase 3 fails → remove scroll snippet from `markdown.js` (existing `data-auto-scroll` logic in Rust still works)

## 13. Open questions

None at design time. The following are explicit future iterations, not open questions for v1:

- Lazy import of additional languages
- Mermaid diagram support
- KaTeX math rendering
- Markdown rendering for user input
- TUI rich text via ratatui

## 14. References

- Existing scroll mechanism: `crates/vol-llm-ui/src/web/components/conversation.rs:183-237` (`[data-scroll]`, `[data-auto-scroll]`)
- Current `ConversationEntry` enum: `crates/vol-llm-ui/src/state/mod.rs:131-142`
- Current rendering: `conversation.rs:267-322` (plain text, `whitespace-pre-wrap`)
- CDN candidates: `https://cdn.jsdelivr.net/npm/marked/marked.min.js`, `https://cdn.jsdelivr.net/npm/dompurify/dist/purify.min.js`, `https://cdn.jsdelivr.net/npm/highlight.js@11/lib/core.min.js`
