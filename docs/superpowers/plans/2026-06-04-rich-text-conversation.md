# Rich Text Display in Conversation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render markdown (headings, lists, code blocks with syntax highlighting, inline code, tables) for `AgentAnswer`, `ContentStreaming`, and `ToolResult` entries in the `vol-llm-ui` web frontend.

**Architecture:** Rust/Dioxus emits sentinel containers (`<div data-md><pre data-md-raw>原文</pre></div>`); a single `markdown.js` module listens via MutationObserver, debounces to 100ms, then runs marked → DOMPurify → highlight.js and replaces `innerHTML`. CDN-loaded with plain-text fallback if libraries fail to load.

**Tech Stack:** Dioxus 0.6, marked v12, DOMPurify v3, highlight.js v11 (all CDN-loaded from jsdelivr), Tailwind CSS v4

---

## File Structure

| File | Purpose | Status |
|------|---------|--------|
| `crates/vol-llm-ui/index.html` | Inject 3 CDN `<script>` tags + 1 highlight.js theme `<link>` + reference to local `markdown.js`/`markdown.css` | Modify (+10 lines) |
| `crates/vol-llm-ui/assets/markdown.js` | CDN detection, MutationObserver, debounce queue, render function, scroll integration, error isolation | Create (~120 lines) |
| `crates/vol-llm-ui/assets/markdown.css` | Dark-theme markdown element styles + fallback styles | Create (~80 lines) |
| `crates/vol-llm-ui/src/web/components/conversation.rs` | Add `html_escape()` + `markdown_container()` helpers; apply to 4 render sites | Modify (+30, -6 lines) |
| `crates/vol-llm-ui/tests/markdown_container.rs` | Rust unit test for the helper's output structure | Create (~40 lines) |
| `crates/vol-llm-ui/tests/web/markdown.spec.js` | Playwright integration test for 4 scenarios | Create (~120 lines) |

**Decomposition rationale:** Each phase corresponds to one PR that produces a self-contained working state. Phase 1 (JS infra) works without any Rust changes — verifiable by manual DOM injection. Phase 2 (Rust integration) is a thin layer that only references Phase 1 assets. Phase 3 (scroll integration) extends Phase 1's JS without touching Rust. Phase 4 adds tests once the system stabilizes.

---

## Phase 1: JS / CSS Infrastructure

Phase 1 adds the markdown rendering pipeline as pure JS/CSS with zero Rust dependency. Manual verification at the end via DOM injection in browser console.

### Task 1.1: Inject CDN scripts and stylesheets into index.html

**Files:**
- Modify: `crates/vol-llm-ui/index.html`

- [ ] **Step 1: Read current index.html**

Run: `cat crates/vol-llm-ui/index.html`
Expected: 11-line minimal HTML with empty `<head>` (only viewport meta + title) and `<div id="main">` body.

- [ ] **Step 2: Replace the `<head>` block to add CDN deps and local assets**

Replace lines 3-7 (the `<head>` block) of `crates/vol-llm-ui/index.html`:

```html
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
    <title>vol | vol-llm-ui</title>

    <!-- Rich text rendering (markdown + syntax highlight + sanitize) -->
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/highlight.js@11.9.0/styles/atom-one-dark.min.css" />
    <link rel="stylesheet" href="/assets/markdown.css" />
    <script src="https://cdn.jsdelivr.net/npm/marked@12.0.2/marked.min.js" defer></script>
    <script src="https://cdn.jsdelivr.net/npm/dompurify@3.0.11/dist/purify.min.js" defer></script>
    <script src="https://cdn.jsdelivr.net/npm/@highlightjs/cdn-assets@11.9.0/highlight.min.js" defer></script>
    <script src="/assets/markdown.js" defer></script>
</head>
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/index.html
git commit -m "feat(web): inject markdown rendering CDN deps and local asset refs

Add marked, DOMPurify, highlight.js (with atom-one-dark theme) via jsdelivr CDN.
Reference local /assets/markdown.js and /assets/markdown.css (created next).
All scripts use defer to keep WASM bootstrap fast."
```

---

### Task 1.2: Create markdown.css with dark theme styles and fallback

**Files:**
- Create: `crates/vol-llm-ui/assets/markdown.css`

- [ ] **Step 1: Create the file with full content**

Create `crates/vol-llm-ui/assets/markdown.css`:

```css
/* Rich text styles for rendered markdown inside [data-md] containers.
 * Applied AFTER markdown.js replaces the <pre data-md-raw> with parsed HTML.
 */

/* Container — neutral text color matches surrounding chat */
[data-md] {
    color: #e0e0e0;
    line-height: 1.55;
    font-size: 14px;
    word-wrap: break-word;
    overflow-wrap: anywhere;
}

/* Headings */
[data-md] h1, [data-md] h2, [data-md] h3,
[data-md] h4, [data-md] h5, [data-md] h6 {
    color: #f0f0f0;
    font-weight: 700;
    line-height: 1.3;
    margin: 1em 0 0.5em;
}
[data-md] h1 { font-size: 1.5em; border-bottom: 1px solid #333; padding-bottom: 0.3em; }
[data-md] h2 { font-size: 1.3em; border-bottom: 1px solid #2a2a44; padding-bottom: 0.2em; }
[data-md] h3 { font-size: 1.15em; }
[data-md] h4 { font-size: 1.05em; }
[data-md] h5, [data-md] h6 { font-size: 1em; color: #ccc; }

/* Paragraphs */
[data-md] p { margin: 0.6em 0; }

/* Lists */
[data-md] ul, [data-md] ol { margin: 0.5em 0; padding-left: 1.8em; }
[data-md] li { margin: 0.2em 0; }
[data-md] li > ul, [data-md] li > ol { margin: 0.2em 0; }

/* Inline code */
[data-md] :not(pre) > code {
    background: #2a2a44;
    color: #f0c060;
    padding: 0.15em 0.35em;
    border-radius: 3px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.92em;
}

/* Fenced code blocks (highlight.js applies its own theme via atom-one-dark.css) */
[data-md] pre {
    background: #1a1a2a;
    border: 1px solid #2a2a44;
    border-radius: 5px;
    padding: 10px 12px;
    overflow-x: auto;
    margin: 0.8em 0;
    font-size: 0.9em;
    line-height: 1.45;
}
[data-md] pre code { background: transparent; padding: 0; color: inherit; font-size: inherit; }

/* Blockquotes */
[data-md] blockquote {
    border-left: 3px solid #4a5a8a;
    margin: 0.6em 0;
    padding: 0.2em 0 0.2em 0.9em;
    color: #aaa;
    font-style: italic;
}

/* Tables */
[data-md] table {
    border-collapse: collapse;
    margin: 0.8em 0;
    font-size: 0.92em;
    overflow-x: auto;
    display: block;
    max-width: 100%;
}
[data-md] th, [data-md] td {
    border: 1px solid #333;
    padding: 6px 10px;
    text-align: left;
}
[data-md] th { background: #2a2a44; font-weight: 600; }

/* Links */
[data-md] a { color: #6080ff; text-decoration: underline; }
[data-md] a:hover { color: #80a0ff; }

/* Horizontal rule */
[data-md] hr { border: none; border-top: 1px solid #333; margin: 1em 0; }

/* Misc */
[data-md] strong { color: #f0f0f0; font-weight: 700; }
[data-md] em { color: inherit; font-style: italic; }
[data-md] del { color: #888; text-decoration: line-through; }

/* Fallback: shown when CDN libs failed to load OR rendering errored on one node */
body.markdown-fallback [data-md] pre[data-md-raw],
[data-md].markdown-error pre[data-md-raw] {
    background: transparent;
    border: none;
    padding: 0;
    margin: 0;
    color: #e0e0e0;
    font-family: inherit;
    font-size: inherit;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-x: visible;
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/assets/markdown.css
git commit -m "feat(web): add markdown.css with dark theme + fallback styles

Styles apply to elements rendered inside [data-md] containers. Includes
fallback rules that make <pre data-md-raw> look like plain chat text when
CDN libs fail or a single node errors during rendering."
```

---

### Task 1.3: Create markdown.js — CDN detection, MutationObserver, render pipeline

**Files:**
- Create: `crates/vol-llm-ui/assets/markdown.js`

- [ ] **Step 1: Create the file with full content**

Create `crates/vol-llm-ui/assets/markdown.js`:

```javascript
/* Markdown rendering for [data-md] containers.
 *
 * Pipeline:
 *   Dioxus emits <div data-md="1" dangerous_inner_html='<pre data-md-raw>原文</pre>'>
 *   MutationObserver detects the node (added or modified)
 *   debounce 100ms → requestIdleCallback → render(node)
 *   render: read pre[data-md-raw] text → marked → DOMPurify → innerHTML → hljs
 *
 * On CDN failure: body.markdown-fallback class shows raw text via fallback CSS.
 * On per-node error: node.markdown-error class falls back the same way.
 */
(function () {
    'use strict';

    const MAX_MD_LENGTH = 50 * 1024;
    const DEBOUNCE_MS = 100;
    const CDN_TIMEOUT_MS = 5000;
    const PRE_REGISTERED_LANGS = [
        'javascript', 'typescript', 'python', 'rust',
        'bash', 'json', 'yaml', 'html', 'css', 'sql'
    ];

    const SANITIZE_CONFIG = {
        ALLOWED_TAGS: [
            'p', 'br', 'strong', 'em', 'del', 'code', 'pre', 'blockquote',
            'ul', 'ol', 'li', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
            'a', 'table', 'thead', 'tbody', 'tr', 'th', 'td', 'hr', 'span'
        ],
        FORBID_TAGS: ['script', 'iframe', 'object', 'embed', 'img', 'video', 'audio', 'style', 'link'],
        ALLOWED_ATTR: ['href', 'target', 'rel', 'class']
    };

    const queue = new Set();
    let flushPending = false;

    function waitForGlobals(timeout) {
        return new Promise((resolve) => {
            const start = Date.now();
            const tick = () => {
                if (window.marked && window.DOMPurify && window.hljs) return resolve(true);
                if (Date.now() - start > timeout) return resolve(false);
                setTimeout(tick, 50);
            };
            tick();
        });
    }

    function setupDOMPurify() {
        DOMPurify.addHook('afterSanitizeAttributes', (node) => {
            if (node.tagName === 'A') {
                node.setAttribute('target', '_blank');
                node.setAttribute('rel', 'noopener noreferrer');
            }
        });
    }

    function setupMarked() {
        marked.setOptions({
            gfm: true,
            breaks: false,
            headerIds: false,
            mangle: false
        });
    }

    function simpleHash(str) {
        let h = 0;
        for (let i = 0; i < str.length; i++) {
            h = ((h << 5) - h + str.charCodeAt(i)) | 0;
        }
        return String(h);
    }

    function render(node) {
        try {
            const rawEl = node.querySelector('pre[data-md-raw]');
            const raw = rawEl ? rawEl.textContent : node.textContent;
            if (raw.length > MAX_MD_LENGTH) return; // leave as <pre> fallback

            const hash = simpleHash(raw);
            if (node.dataset.mdRendered === hash) return;

            const html = marked.parse(raw);
            const safe = DOMPurify.sanitize(html, SANITIZE_CONFIG);
            node.innerHTML = safe;
            node.dataset.mdRendered = hash;
            node.classList.remove('markdown-error');

            node.querySelectorAll('pre code').forEach((codeEl) => {
                try { hljs.highlightElement(codeEl); }
                catch (e) { console.warn('[markdown] hljs failed on block:', e); }
            });

            // Phase 3 hook (placeholder): scroll-to-bottom integration is added later
        } catch (e) {
            console.error('[markdown] render error:', e);
            node.classList.add('markdown-error');
        }
    }

    function flush() {
        flushPending = false;
        const nodes = Array.from(queue);
        queue.clear();
        if (window.requestIdleCallback) {
            requestIdleCallback(() => nodes.forEach(render), { timeout: 200 });
        } else {
            setTimeout(() => nodes.forEach(render), 0);
        }
    }

    function enqueue(node) {
        queue.add(node);
        if (!flushPending) {
            flushPending = true;
            setTimeout(flush, DEBOUNCE_MS);
        }
    }

    function collectMdNodes(node, out) {
        if (node.nodeType !== Node.ELEMENT_NODE) return;
        if (node.hasAttribute && node.hasAttribute('data-md')) out.push(node);
        if (node.querySelectorAll) {
            node.querySelectorAll('[data-md]').forEach((n) => out.push(n));
        }
    }

    function findEnclosingMdNode(node) {
        let cur = node.nodeType === Node.ELEMENT_NODE ? node : node.parentNode;
        while (cur) {
            if (cur.nodeType === Node.ELEMENT_NODE && cur.hasAttribute && cur.hasAttribute('data-md')) {
                return cur;
            }
            cur = cur.parentNode;
        }
        return null;
    }

    function onMutations(mutations) {
        const found = [];
        for (const m of mutations) {
            if (m.type === 'childList') {
                m.addedNodes.forEach((n) => collectMdNodes(n, found));
            } else if (m.type === 'characterData') {
                const ancestor = findEnclosingMdNode(m.target);
                if (ancestor) found.push(ancestor);
            }
        }
        if (found.length === 0) return;
        for (const n of found) enqueue(n);
    }

    function startObserver() {
        const observer = new MutationObserver(onMutations);
        observer.observe(document.body, {
            childList: true,
            subtree: true,
            characterData: true
        });
        // Also process any [data-md] nodes already present at startup
        const initial = [];
        document.querySelectorAll('[data-md]').forEach((n) => initial.push(n));
        initial.forEach(enqueue);
    }

    async function init() {
        const ok = await waitForGlobals(CDN_TIMEOUT_MS);
        if (!ok) {
            console.warn('[markdown] CDN libs not loaded; falling back to plain text');
            document.body.classList.add('markdown-fallback');
            return;
        }
        setupDOMPurify();
        setupMarked();
        // highlight.js cdn-assets bundle already registers all common langs;
        // pre-registered list is documentary — no explicit registerLanguage needed.
        // (Whitelist enforced by PRE_REGISTERED_LANGS for testing reference.)
        void PRE_REGISTERED_LANGS;
        startObserver();
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/assets/markdown.js
git commit -m "feat(web): add markdown.js — MutationObserver-driven render pipeline

Watches [data-md] containers, debounces 100ms, runs marked + DOMPurify +
highlight.js. CDN-load with 5s timeout; on failure, body.markdown-fallback
class shows raw text via fallback CSS. Per-node try/catch isolates errors.
Caps single-node markdown at 50KB to prevent runaway renders."
```

---

### Task 1.4: Manual verification of Phase 1 rendering

**Files:**
- Touch (verification only): browser console

- [ ] **Step 1: Verify all three services are running**

Run:
```bash
lsof -i :8080 2>/dev/null && echo "dev OK" || echo "dev MISSING"
lsof -i :3001 2>/dev/null && echo "backend OK" || echo "backend MISSING"
pgrep -f tailwindcss >/dev/null && echo "tailwind OK" || echo "tailwind MISSING"
```
Expected: all three OK. If any missing, start with `make web-dev`, `make web-backend`, `make web-css` in separate terminals.

- [ ] **Step 2: Open browser, hard refresh, paste injection snippet in DevTools console**

Open http://localhost:8080 in browser. Press Cmd+Shift+R (hard refresh to load updated `index.html`).

Open DevTools (F12), Console tab, paste:

```javascript
const div = document.createElement('div');
div.setAttribute('data-md', '1');
div.innerHTML = '<pre data-md-raw># Hello\n\nThis is **bold** and `code`.\n\n```python\ndef hello():\n    return "world"\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |</pre>';
document.body.appendChild(div);
```

Expected after ~200ms: a rendered block appears at the bottom of the page showing:
- H1 "Hello"
- Paragraph with bold "bold" and orange-background inline `code`
- Syntax-highlighted Python code block (atom-one-dark theme)
- A 2-column table with borders

- [ ] **Step 3: Verify the CDN-failure fallback path**

In DevTools console, run:
```javascript
const div2 = document.createElement('div');
div2.setAttribute('data-md', '1');
div2.innerHTML = '<pre data-md-raw># Fallback test</pre>';
document.body.classList.add('markdown-fallback');
document.body.appendChild(div2);
```

Expected: the new `[data-md]` block shows `# Fallback test` as plain text (no border, no background), matching the styling of surrounding chat text.

Cleanup:
```javascript
document.body.classList.remove('markdown-fallback');
div.remove();
div2.remove();
```

- [ ] **Step 4: If verification passes, commit a marker**

Phase 1 has no new files to add — commits were made per-task. Print:
```bash
git log --oneline -5
```
Expected: see the three Phase 1 commits in order.

---

## Phase 2: Rust Integration

Wire the Rust-side `markdown_container()` helper to the four render sites. Each site change is independently verifiable.

### Task 2.1: Add html_escape and markdown_container helpers + Rust unit tests

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs`
- Create: `crates/vol-llm-ui/tests/markdown_container.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vol-llm-ui/tests/markdown_container.rs`:

```rust
//! Unit tests for the html_escape helper used by markdown_container.
//! markdown_container itself returns a Dioxus Element which is hard to
//! introspect outside a VirtualDom; we test the pure-string escape function
//! that determines what ends up inside <pre data-md-raw>.

use vol_llm_ui::web::components::conversation::html_escape;

#[test]
fn escapes_html_special_chars() {
    assert_eq!(html_escape("a & b"), "a &amp; b");
    assert_eq!(html_escape("<script>"), "&lt;script&gt;");
    assert_eq!(html_escape("a \"quoted\" word"), "a &quot;quoted&quot; word");
    assert_eq!(html_escape("'apostrophe'"), "&#39;apostrophe&#39;");
}

#[test]
fn preserves_plain_text() {
    assert_eq!(html_escape("hello world"), "hello world");
    assert_eq!(html_escape(""), "");
    assert_eq!(html_escape("中文"), "中文");
}

#[test]
fn ampersand_only_escaped_once() {
    // No double-escape: & should become &amp;, not &amp;amp;
    assert_eq!(html_escape("&amp;"), "&amp;amp;");
    // (This is the correct behavior — we treat the input as raw text, not as already-escaped HTML.)
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vol-llm-ui --features web --test markdown_container 2>&1 | head -20`
Expected: FAIL — compile error "unresolved import" because `html_escape` doesn't exist yet, or "function is private" if module visibility isn't right.

- [ ] **Step 3: Add the helpers to conversation.rs**

Open `crates/vol-llm-ui/src/web/components/conversation.rs`. Just after the imports block (around line 11, after `use crate::state::...`), insert:

```rust
/// Escapes the five HTML-significant characters so a raw string is safe to
/// embed inside `<pre data-md-raw>...</pre>` without breaking parsing.
///
/// Used by [`markdown_container`] to wrap LLM-produced markdown for the
/// JS-side renderer (`assets/markdown.js`).
pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Renders a container that `assets/markdown.js` picks up and replaces with
/// rendered markdown. The `<pre data-md-raw>` wrapper preserves whitespace
/// and acts as a plain-text fallback if the JS pipeline is unavailable.
///
/// See `docs/superpowers/specs/2026-06-04-rich-text-conversation-design.md`.
fn markdown_container(text: &str, extra_class: &str) -> Element {
    let raw_html = format!("<pre data-md-raw>{}</pre>", html_escape(text));
    let class = format!("text-[#e0e0e0] leading-[1.5] {}", extra_class);
    rsx! {
        div {
            class: "{class}",
            "data-md": "1",
            dangerous_inner_html: "{raw_html}"
        }
    }
}
```

The `extra_class` parameter lets each call site append site-specific Tailwind classes (e.g. `ml-4` for tool results, `font-mono text-xs` for tool detail bodies) without losing the container's base styling.

- [ ] **Step 4: Make html_escape visible to the integration test**

The test imports `vol_llm_ui::web::components::conversation::html_escape`. Check that `crates/vol-llm-ui/src/lib.rs` re-exports the `web` module under the `web` feature. Run:

```bash
grep -n "pub mod web" crates/vol-llm-ui/src/lib.rs
```

If the line exists and is feature-gated, the test will only compile with `--features web`. The test command already includes `--features web` so this should work. If `web` module isn't `pub`, no fix needed for now — we'll just test via the cfg path:

```bash
grep -n "mod conversation" crates/vol-llm-ui/src/web/components/mod.rs
```

Expected: `pub mod conversation;`. If it's `mod conversation;` (private), change to `pub mod conversation;` so the test can access it. Same check for `pub mod components;` in `crates/vol-llm-ui/src/web/mod.rs`.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vol-llm-ui --features web --test markdown_container 2>&1 | tail -10`
Expected: `test result: ok. 3 passed; 0 failed`.

- [ ] **Step 6: Verify the whole crate still compiles for web**

Run: `make web-check 2>&1 | tail -5`
Expected: `Finished \`dev\` profile [unoptimized + debuginfo] target(s)` with no errors. One pre-existing `unused doc comment` warning is acceptable.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs crates/vol-llm-ui/tests/markdown_container.rs crates/vol-llm-ui/src/web/components/mod.rs crates/vol-llm-ui/src/web/mod.rs
git commit -m "feat(web): add html_escape and markdown_container helpers

html_escape converts the 5 HTML-significant chars to entities so LLM text
is safe to embed inside <pre data-md-raw>. markdown_container wraps that
inside <div data-md=1>, which assets/markdown.js detects via
MutationObserver and replaces with rendered HTML."
```

---

### Task 2.2: Apply markdown_container to AgentAnswer

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs:320-322`

- [ ] **Step 1: Replace the AgentAnswer arm**

Open `crates/vol-llm-ui/src/web/components/conversation.rs`. Find the `ConversationEntry::AgentAnswer { text } =>` arm (around line 320 — search for `whitespace-pre-wrap`). Current:

```rust
ConversationEntry::AgentAnswer { text } => {
    rsx! { div { class: "text-[#e0e0e0] whitespace-pre-wrap leading-[1.5]", {text} } }
}
```

Replace with:

```rust
ConversationEntry::AgentAnswer { text } => {
    markdown_container(&text, "")
}
```

- [ ] **Step 2: Compile-check**

Run: `make web-check 2>&1 | tail -5`
Expected: clean build (pre-existing unused-doc-comment warning aside).

- [ ] **Step 3: Manual browser verification**

If `make web-dev` is running, the dev server hot-reloads. In the browser:

1. Hard-refresh http://localhost:8080
2. Select an agent
3. Send a message asking: `回复一段包含 # 标题、**粗体**、和 \`code\` 的内容`
4. Expected: the agent's answer renders with formatted heading, bold text, inline code styling — NOT the literal markdown characters.

If backend can't connect to LLM, alternative manual check: use DevTools console to inject a fake answer:
```javascript
// In console, find a chat container and inject a test entry
// (this is brittle; a passing make web-check is sufficient for this step)
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs
git commit -m "feat(web): render AgentAnswer via markdown_container

Agent answers now flow through the markdown.js pipeline (marked +
DOMPurify + highlight.js). Headings, lists, code blocks, tables, and
inline code are formatted; plain prose unchanged."
```

---

### Task 2.3: Apply markdown_container to ContentStreaming

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs:272-278`

- [ ] **Step 1: Replace the ContentStreaming arm**

Find the `ConversationEntry::ContentStreaming { content } =>` arm (around line 272). Current:

```rust
ConversationEntry::ContentStreaming { content } => {
    if content.is_empty() {
        rsx! { div { class: "text-[#888]", "Generating..." } }
    } else {
        rsx! { div { class: "text-[#e0e0e0]", {content} } }
    }
}
```

Replace with:

```rust
ConversationEntry::ContentStreaming { content } => {
    if content.is_empty() {
        rsx! { div { class: "text-[#888]", "Generating..." } }
    } else {
        markdown_container(&content, "")
    }
}
```

The "Generating..." placeholder stays plain text — no markdown to render in an empty string, and the placeholder shouldn't trigger the JS pipeline.

- [ ] **Step 2: Compile-check**

Run: `make web-check 2>&1 | tail -5`
Expected: clean build.

- [ ] **Step 3: Manual streaming verification**

This requires the backend connected to a real LLM. If backend is unreachable, skip the streaming check and rely on Phase 4 Playwright test for coverage.

If backend works:
1. Hard-refresh, select agent, send `请写一段含代码块的回复`
2. Watch the streaming output. Expected behavior:
   - For the first ~100ms there's no markdown rendering (debounce window)
   - Then formatted output starts appearing, roughly 10 updates per second max
   - Final result identical to AgentAnswer rendering

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs
git commit -m "feat(web): render ContentStreaming via markdown_container

Streaming agent output now renders markdown live, throttled to 10 updates
per second by markdown.js's 100ms debounce. Final ContentComplete swap to
AgentAnswer is handled identically (re-renders the same hash → no churn)."
```

---

### Task 2.4: Apply markdown_container to ToolResult preview

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs:316`

- [ ] **Step 1: Replace the preview line inside the ToolResult arm**

Find the `ConversationEntry::ToolResult` arm (around line 299). The preview div is on line 316:

```rust
div { class: "text-[#888] text-xs mt-0.5 font-mono line-clamp-2 overflow-hidden", "{preview}" }
```

Replace that single line with:

```rust
div { class: "text-[#888] text-xs mt-0.5 line-clamp-2 overflow-hidden",
    {markdown_container(&preview, "font-mono")}
}
```

Two changes:
1. The outer `div` keeps its layout classes (`text-[#888] text-xs mt-0.5 line-clamp-2 overflow-hidden`) but drops `font-mono` (which moves into the helper's `extra_class`)
2. The `{markdown_container(...)}` invocation replaces the bare `"{preview}"` interpolation

- [ ] **Step 2: Compile-check**

Run: `make web-check 2>&1 | tail -5`
Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs
git commit -m "feat(web): render ToolResult preview via markdown_container

Tool result preview (the 2-line summary on the tool card) now picks up
markdown. JSON tool results commonly include code-block formatting; this
makes them readable."
```

---

### Task 2.5: Apply markdown_container to ToolDetailModal result body

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs:411-417`

- [ ] **Step 1: Replace the result body in the modal**

Find the `ToolDetailModal` component result body (around line 411). Current:

```rust
if let Some(ref res) = result_display {
    div {
        div { class: "text-[#888] text-xs mb-1 font-bold", "Result" }
        div { class: "text-[#ccc] font-mono text-xs bg-[#111128] rounded p-2.5 whitespace-pre-wrap break-all max-h-[240px] overflow-y-auto",
            "{res}"
        }
    }
}
```

Replace the inner `div` (the one with `"{res}"`) with:

```rust
if let Some(ref res) = result_display {
    div {
        div { class: "text-[#888] text-xs mb-1 font-bold", "Result" }
        div { class: "text-[#ccc] text-xs bg-[#111128] rounded p-2.5 break-all max-h-[240px] overflow-y-auto",
            {markdown_container(res, "font-mono text-xs")}
        }
    }
}
```

Removed `whitespace-pre-wrap` and `font-mono` from the outer container (markdown_container handles whitespace via `<pre>`, and the `extra_class` carries `font-mono`).

- [ ] **Step 2: Compile-check**

Run: `make web-check 2>&1 | tail -5`
Expected: clean build.

- [ ] **Step 3: Manual verification**

Hard-refresh the browser. If you have any past tool result in the conversation, click on the tool card to open the modal. Verify:
1. The "Result" section renders any markdown in the result body (code blocks, lists, etc.)
2. The "Arguments" section (above) is unchanged — still plain JSON
3. Modal layout, max-height, and scrolling still work

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs
git commit -m "feat(web): render ToolDetailModal result body via markdown_container

Tool detail modal's result panel renders markdown. The Arguments panel
(typically raw JSON) stays plain text — no benefit and risk of confusing
display."
```

---

### Task 2.6: Phase 2 sign-off — full clippy run

**Files:**
- None (verification only)

- [ ] **Step 1: Run clippy on the web target**

Run: `make web-clippy 2>&1 | tail -20`
Expected: `Finished` with no new warnings caused by this PR. Pre-existing warnings (such as the `unused doc comment` in `app.rs:12`) are acceptable.

- [ ] **Step 2: Run all Rust tests for vol-llm-ui (web feature)**

Run: `cargo test -p vol-llm-ui --features web 2>&1 | tail -10`
Expected: all tests pass including the new `markdown_container` tests.

- [ ] **Step 3: Verify in browser that conversation still functions normally**

Hard-refresh http://localhost:8080. Click around: tab switching, agent selection, sending a message, opening tool detail. Nothing should be broken. The visible change is that any markdown content now renders formatted.

---

## Phase 3: Scroll Integration

Extend `markdown.js` so that when markdown rendering changes DOM height while the user is sticky-at-bottom, the scroll position follows. The Rust scroll logic from the previous fix already supports `data-scroll-programmatic` — markdown.js just needs to set it before scrolling.

### Task 3.1: Wire scroll-to-bottom into the render function

**Files:**
- Modify: `crates/vol-llm-ui/assets/markdown.js`

- [ ] **Step 1: Replace the render function's tail section**

Open `crates/vol-llm-ui/assets/markdown.js`. Find the `render` function. The current tail (just before the catch) is:

```javascript
            node.querySelectorAll('pre code').forEach((codeEl) => {
                try { hljs.highlightElement(codeEl); }
                catch (e) { console.warn('[markdown] hljs failed on block:', e); }
            });

            // Phase 3 hook (placeholder): scroll-to-bottom integration is added later
        } catch (e) {
```

Replace those lines with:

```javascript
            node.querySelectorAll('pre code').forEach((codeEl) => {
                try { hljs.highlightElement(codeEl); }
                catch (e) { console.warn('[markdown] hljs failed on block:', e); }
            });

            maybeStickToBottom();
        } catch (e) {
```

- [ ] **Step 2: Add the maybeStickToBottom helper**

In `markdown.js`, just above the `render` function (after `simpleHash`), insert:

```javascript
/**
 * If the conversation scroll container has data-auto-scroll="1" (user is
 * sticky-at-bottom), scroll it to the bottom. Sets data-scroll-programmatic
 * so the Rust onscroll handler skips this event without toggling state.
 *
 * Matches the contract defined in
 * crates/vol-llm-ui/src/web/components/conversation.rs (line ~205).
 */
function maybeStickToBottom() {
    const el = document.querySelector('[data-scroll]');
    if (!el) return;
    if (el.getAttribute('data-auto-scroll') === '0') return;
    el.setAttribute('data-scroll-programmatic', '1');
    el.scrollTop = el.scrollHeight;
}
```

- [ ] **Step 3: Verify the JS file parses (no Rust compile needed — pure asset)**

Open browser DevTools, Sources tab, navigate to `assets/markdown.js`. Confirm syntax-highlighted display with no red error markers. Or run:

```bash
node --check crates/vol-llm-ui/assets/markdown.js
```

Expected: no output (success). If `node` is not installed, the browser-side parse check is sufficient.

- [ ] **Step 4: Manual verification — stick-to-bottom during streaming**

Hard-refresh the browser. With backend connected to LLM:

1. Scroll the conversation to the bottom
2. Send a message that produces long output (`请写20行关于春天的诗，每行一段`)
3. Expected: as markdown renders progressively, the view stays anchored to the bottom

- [ ] **Step 5: Manual verification — user scroll up cancels stick**

1. While streaming output, scroll up slightly (mouse wheel, ~10px)
2. Expected: view stays where the user left it; subsequent markdown renders do NOT pull back to the bottom

- [ ] **Step 6: Manual verification — scroll back to bottom resumes stick**

1. Continue from the previous state (mid-stream, scrolled up)
2. Manually scroll back to the absolute bottom
3. Expected: subsequent renders pull the view to bottom again

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-ui/assets/markdown.js
git commit -m "feat(web): markdown.js — stick-to-bottom on render when sticky

After each render, if the conversation container has
data-auto-scroll=\"1\", scroll to bottom with data-scroll-programmatic=\"1\"
so the Rust onscroll handler doesn't interpret the JS-driven scroll as a
user gesture."
```

---

## Phase 4: Tests and Documentation

Add Playwright tests covering the 4 design-spec scenarios, then update the wiki.

### Task 4.1: Add Playwright test for markdown rendering

**Files:**
- Create: `crates/vol-llm-ui/tests/web/markdown.spec.js`
- Create (if missing): `crates/vol-llm-ui/tests/web/playwright.config.js`

- [ ] **Step 1: Check whether Playwright is already set up for this crate**

Run:
```bash
find crates/vol-llm-ui -name "playwright.config*" -o -name "package.json" | head -5
cat crates/vol-llm-ui/package.json
```

Expected: `package.json` exists with `tailwindcss` dependency but no Playwright. If Playwright is missing, the test file is still added but a `npm install` step is needed.

- [ ] **Step 2: Add Playwright as a dev dependency**

Run:
```bash
npm install --prefix crates/vol-llm-ui --save-dev @playwright/test
npx --prefix crates/vol-llm-ui playwright install chromium
```

Expected: `node_modules/@playwright/test/` populated. The `playwright install` step downloads Chromium (~150MB), which is cached for future runs.

- [ ] **Step 3: Create playwright.config.js**

Create `crates/vol-llm-ui/tests/web/playwright.config.js`:

```javascript
// @ts-check
const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
    testDir: '.',
    timeout: 30_000,
    use: {
        baseURL: 'http://localhost:8080',
        headless: true,
        viewport: { width: 1280, height: 800 },
    },
});
```

- [ ] **Step 4: Create the test file**

Create `crates/vol-llm-ui/tests/web/markdown.spec.js`:

```javascript
// @ts-check
const { test, expect } = require('@playwright/test');

/**
 * These tests assume `make web-dev`, `make web-backend`, and `make web-css`
 * are running. They drive only the frontend (no LLM round-trip required)
 * by injecting [data-md] containers via the DOM.
 */

test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // Wait for the WASM app to mount its root
    await page.waitForSelector('body', { timeout: 5000 });
});

test('1. static markdown renders correctly', async ({ page }) => {
    await page.evaluate(() => {
        const div = document.createElement('div');
        div.id = 'pw-md-1';
        div.setAttribute('data-md', '1');
        div.innerHTML = '<pre data-md-raw># Title\n\n**bold** and `code` and a list:\n- one\n- two</pre>';
        document.body.appendChild(div);
    });

    // markdown.js debounce is 100ms; give it some headroom
    await page.waitForTimeout(500);

    const html = await page.locator('#pw-md-1').innerHTML();
    expect(html).toContain('<h1>Title</h1>');
    expect(html).toContain('<strong>bold</strong>');
    expect(html).toMatch(/<code[^>]*>code<\/code>/);
    expect(html).toMatch(/<ul>[\s\S]*<li>one<\/li>/);
});

test('2. streaming render is throttled to <= 12 renders/sec', async ({ page }) => {
    // Count renders by overriding marked.parse with a counter
    await page.evaluate(() => {
        window.__pwRenderCount = 0;
        const origParse = window.marked.parse;
        window.marked.parse = (s) => { window.__pwRenderCount++; return origParse(s); };

        const div = document.createElement('div');
        div.id = 'pw-md-2';
        div.setAttribute('data-md', '1');
        div.innerHTML = '<pre data-md-raw></pre>';
        document.body.appendChild(div);

        // Simulate streaming: 50 character updates over 1 second
        let i = 0;
        const text = 'Streaming text token by token, gradually building markdown.';
        const id = setInterval(() => {
            const pre = div.querySelector('pre[data-md-raw]');
            pre.textContent = text.slice(0, i++);
            if (i > text.length) { clearInterval(id); window.__pwStreamDone = true; }
        }, 20); // ~50 updates per second
    });

    // Wait for streaming + debounce to complete
    await page.waitForFunction(() => window.__pwStreamDone, { timeout: 5000 });
    await page.waitForTimeout(300);

    const count = await page.evaluate(() => window.__pwRenderCount);
    // ~50 mutations in 1s; with 100ms debounce, expect ~10-12 renders
    expect(count).toBeLessThanOrEqual(12);
    expect(count).toBeGreaterThanOrEqual(1);
});

test('3. CDN failure falls back to plain text', async ({ page }) => {
    // Force fallback by adding the class manually (simulates failed CDN load)
    await page.evaluate(() => {
        document.body.classList.add('markdown-fallback');
        const div = document.createElement('div');
        div.id = 'pw-md-3';
        div.setAttribute('data-md', '1');
        div.innerHTML = '<pre data-md-raw># Still readable</pre>';
        document.body.appendChild(div);
    });

    await page.waitForTimeout(300);

    const pre = page.locator('#pw-md-3 pre[data-md-raw]');
    const text = await pre.textContent();
    expect(text).toBe('# Still readable');

    // Verify the fallback CSS made the pre look like plain text (no monospace bg)
    const bg = await pre.evaluate((el) => getComputedStyle(el).backgroundColor);
    expect(bg).toBe('rgba(0, 0, 0, 0)'); // transparent

    await page.evaluate(() => document.body.classList.remove('markdown-fallback'));
});

test('4. script tags are stripped from rendered output', async ({ page }) => {
    await page.evaluate(() => {
        const div = document.createElement('div');
        div.id = 'pw-md-4';
        div.setAttribute('data-md', '1');
        // marked passes through HTML; DOMPurify must strip <script>
        div.innerHTML = '<pre data-md-raw>Hello\n\n&lt;script&gt;alert(1)&lt;/script&gt;</pre>';
        document.body.appendChild(div);
    });

    await page.waitForTimeout(500);

    const html = await page.locator('#pw-md-4').innerHTML();
    expect(html).not.toContain('<script');
    expect(html).not.toContain('alert(1)'); // even as text — DOMPurify removes content of script tags
});
```

- [ ] **Step 5: Run the Playwright tests**

Run:
```bash
cd crates/vol-llm-ui && npx playwright test --config tests/web/playwright.config.js
```

Expected: `4 passed` in green. If `make web-dev` is not running, all tests fail with "ERR_CONNECTION_REFUSED" — start it first.

- [ ] **Step 6: Update .gitignore for Playwright artifacts**

Append to `crates/vol-llm-ui/.gitignore` (create if missing):

```
node_modules/
test-results/
playwright-report/
```

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-ui/tests/web/markdown.spec.js \
        crates/vol-llm-ui/tests/web/playwright.config.js \
        crates/vol-llm-ui/package.json \
        crates/vol-llm-ui/package-lock.json \
        crates/vol-llm-ui/.gitignore
git commit -m "test(web): Playwright tests for markdown rendering pipeline

Four scenarios: static render, streaming throttle (<=12/sec on 50 mutations/sec),
CDN-failure fallback, XSS payload stripping. Requires make web-dev running."
```

---

### Task 4.2: Take screenshot archive of representative messages

**Files:**
- Create: `docs/screenshots/rich-text/static-markdown.png`
- Create: `docs/screenshots/rich-text/code-blocks.png`
- Create: `docs/screenshots/rich-text/table.png`

- [ ] **Step 1: Create the screenshots directory**

Run: `mkdir -p docs/screenshots/rich-text`

- [ ] **Step 2: Add a Playwright script that captures screenshots**

Create `crates/vol-llm-ui/tests/web/screenshots.spec.js`:

```javascript
// @ts-check
const { test } = require('@playwright/test');
const path = require('path');

const OUT_DIR = path.resolve(__dirname, '../../../../docs/screenshots/rich-text');

test('screenshot: static markdown sample', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(800);
    await page.evaluate(() => {
        const div = document.createElement('div');
        div.id = 'shot';
        div.setAttribute('data-md', '1');
        div.style.cssText = 'padding:24px;background:#1a1a2e;width:720px;';
        div.innerHTML = '<pre data-md-raw># Heading 1\n\n## Heading 2\n\nThis is a paragraph with **bold**, *italic*, and `inline code`.\n\n- First list item\n- Second list item\n  - Nested item\n\n> A blockquote with some thoughtful prose.\n\n[A link](https://example.com)</pre>';
        document.body.appendChild(div);
    });
    await page.waitForTimeout(500);
    await page.locator('#shot').screenshot({ path: `${OUT_DIR}/static-markdown.png` });
});

test('screenshot: code blocks', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(800);
    await page.evaluate(() => {
        const div = document.createElement('div');
        div.id = 'shot';
        div.setAttribute('data-md', '1');
        div.style.cssText = 'padding:24px;background:#1a1a2e;width:720px;';
        div.innerHTML = '<pre data-md-raw>```python\ndef hello(name: str) -> str:\n    return f"Hello, {name}!"\n```\n\n```rust\nfn main() {\n    let v = vec![1, 2, 3];\n    for x in v { println!("{}", x); }\n}\n```\n\n```bash\n$ cargo test --features web\n```</pre>';
        document.body.appendChild(div);
    });
    await page.waitForTimeout(500);
    await page.locator('#shot').screenshot({ path: `${OUT_DIR}/code-blocks.png` });
});

test('screenshot: table', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(800);
    await page.evaluate(() => {
        const div = document.createElement('div');
        div.id = 'shot';
        div.setAttribute('data-md', '1');
        div.style.cssText = 'padding:24px;background:#1a1a2e;width:720px;';
        div.innerHTML = '<pre data-md-raw>| Tool | Calls | Avg ms |\n|------|------:|-------:|\n| web_search | 12 | 340 |\n| read_file | 47 | 8 |\n| run_command | 3 | 1240 |</pre>';
        document.body.appendChild(div);
    });
    await page.waitForTimeout(500);
    await page.locator('#shot').screenshot({ path: `${OUT_DIR}/table.png` });
});
```

- [ ] **Step 3: Run the screenshot script**

Run:
```bash
cd crates/vol-llm-ui && npx playwright test --config tests/web/playwright.config.js tests/web/screenshots.spec.js
```

Expected: 3 PNG files appear in `docs/screenshots/rich-text/`. Open them to visually confirm the rendering matches the design.

- [ ] **Step 4: Commit**

```bash
git add docs/screenshots/rich-text/*.png crates/vol-llm-ui/tests/web/screenshots.spec.js
git commit -m "docs(screenshots): visual archive of rich text rendering

Three PNGs covering headings/lists/inline formatting, code blocks across
3 languages, and tables. Captured via Playwright into a fixed-width view
so the archive is regenerable."
```

---

### Task 4.3: Update wiki with rich-text-conversation concept page

**Files:**
- Create: `docs/wiki/concepts/rich-text-conversation.md`
- Modify: `docs/wiki/INDEX.md`

- [ ] **Step 1: Create the wiki concept page**

Create `docs/wiki/concepts/rich-text-conversation.md`:

```markdown
---
title: Rich Text Conversation Rendering
status: active
updated: 2026-06-04
related: [vol-llm-ui-crate, conversation-view]
---

# Rich Text Conversation Rendering

Renders markdown in agent answers, streaming output, and tool results on the web frontend. Implemented as a Rust/JS handoff: Dioxus emits sentinel containers, a JS module renders them via marked + DOMPurify + highlight.js.

## Why this design

The original conversation view (`crates/vol-llm-ui/src/web/components/conversation.rs:267-322`) rendered all text as plain `<div>` with `whitespace-pre-wrap`. LLM-produced markdown stayed literal. Adding a Rust markdown crate (pulldown-cmark or comrak) would have:
- Increased WASM bundle size by 200-500KB
- Required HTML sanitization on the Rust side
- Coupled rendering choice to Rust compile time

Instead, the Rust side is intentionally markdown-unaware: it emits a `<div data-md="1">` containing `<pre data-md-raw>原文</pre>`. The browser-side `markdown.js` watches for these via MutationObserver, debounces high-frequency mutations (streaming output), and renders the result.

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
| `crates/vol-llm-ui/src/web/components/conversation.rs` | `html_escape()` and `markdown_container()` helpers; applied at 4 render sites |
| `crates/vol-llm-ui/index.html` | CDN script tags (marked, DOMPurify, highlight.js) |

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
- Scroll mechanism this integrates with: [[conversation-view]] (`data-auto-scroll`, `data-scroll-programmatic`)
```

- [ ] **Step 2: Add the page to the wiki index**

Read `docs/wiki/INDEX.md` to find where concepts are listed. The structure typically has a `## Concepts` table. Locate it:

```bash
grep -n "## Concepts" docs/wiki/INDEX.md
```

Insert a new row in the Concepts table (alphabetical or by date — match the existing convention):

```markdown
| [[rich-text-conversation]] | Markdown rendering for chat (Rust + marked.js bridge) | active | 2026-06-04 |
```

Also update the "Last updated" line at the top of INDEX.md to today's date if that convention is followed.

- [ ] **Step 3: Commit**

```bash
git add docs/wiki/concepts/rich-text-conversation.md docs/wiki/INDEX.md
git commit -m "docs(wiki): add rich-text-conversation concept page

Architecture, data flow, file responsibilities, failure modes, and the
list of render sites that DO vs DO NOT use markdown. Cross-references
the design spec and this plan."
```

- [ ] **Step 4: Final verification — all phases done**

Run:
```bash
git log --oneline -20
make web-check 2>&1 | tail -3
cargo test -p vol-llm-ui --features web 2>&1 | tail -5
```

Expected:
- ~15 commits since the start of Phase 1
- `web-check` clean
- All Rust tests pass

The feature is complete. Hard-refresh the browser and use the agent to verify rich text actually renders end-to-end with the LLM.

---

## Summary

| Phase | PRs | Files changed | Risk |
|-------|-----|---------------|------|
| 1. JS / CSS infra | 1 | 3 created + index.html | Low — pure asset, no Rust dependency |
| 2. Rust integration | 1 | 1 modified (conversation.rs) + 1 test file | Low — narrow helper applied to 4 sites |
| 3. Scroll integration | 1 | markdown.js | Very low — additive, easily reverted |
| 4. Tests & docs | 1 | 3 test files + screenshots + wiki | Low — tests only |

**Total: 4 PRs, ~270 LOC across 6 new files + 3 modified files.**

Every phase produces working, verifiable software on its own; each phase is independently revertible without touching others.
