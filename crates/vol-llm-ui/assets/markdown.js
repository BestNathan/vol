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

    /**
     * If the conversation scroll container has data-auto-scroll="1" (user is
     * sticky-at-bottom), scroll it to the bottom. Sets data-scroll-programmatic
     * so the Rust onscroll handler skips this event without toggling state.
     */
    function maybeStickToBottom() {
        const el = document.querySelector('[data-scroll]');
        if (!el) return;
        if (el.getAttribute('data-auto-scroll') === '0') return;
        el.setAttribute('data-scroll-programmatic', '1');
        el.scrollTop = el.scrollHeight;
    }

    function render(node) {
        try {
            const rawEl = node.querySelector('pre[data-md-raw]');
            const raw = rawEl ? rawEl.textContent : node.textContent;
            if (raw.length > MAX_MD_LENGTH) return;

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

            maybeStickToBottom();
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
        void PRE_REGISTERED_LANGS;
        startObserver();
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
