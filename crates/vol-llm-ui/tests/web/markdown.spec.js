// @ts-check
const { test, expect } = require('@playwright/test');

async function waitForWasm(page) {
    await page.waitForSelector('body', { timeout: 15_000 });
    // Give the WASM bootstrap time to complete
    await page.waitForTimeout(3000);
}

test.beforeEach(async ({ page }) => {
    await page.goto('/', { timeout: 30_000, waitUntil: 'domcontentloaded' });
    await waitForWasm(page);
});

test('1. static markdown renders correctly', async ({ page }) => {
    // Wait for CDN libs to load
    await page.waitForFunction(() => !!window.marked, { timeout: 10_000 });

    await page.evaluate(() => {
        const div = document.createElement('div');
        div.id = 'pw-md-1';
        div.setAttribute('data-md', '1');
        div.innerHTML = '<pre data-md-raw># Title\n\n**bold** and `code` and a list:\n- one\n- two</pre>';
        document.body.appendChild(div);
    });

    await page.waitForTimeout(800);

    const html = await page.locator('#pw-md-1').innerHTML();
    expect(html).toContain('<h1>Title</h1>');
    expect(html).toContain('<strong>bold</strong>');
    expect(html).toMatch(/<code[^>]*>code<\/code>/);
    expect(html).toMatch(/<ul>[\s\S]*<li>one<\/li>/);
});

test('2. streaming render is throttled to <= 12 renders/sec', async ({ page }) => {
    await page.waitForFunction(() => !!window.marked, { timeout: 10_000 });

    await page.evaluate(() => {
        window.__pwRenderCount = 0;
        const origParse = window.marked.parse;
        window.marked.parse = (s) => { window.__pwRenderCount++; return origParse(s); };

        const div = document.createElement('div');
        div.id = 'pw-md-2';
        div.setAttribute('data-md', '1');
        div.innerHTML = '<pre data-md-raw></pre>';
        document.body.appendChild(div);

        let i = 0;
        const text = 'Streaming text token by token, gradually building markdown.';
        const id = setInterval(() => {
            const pre = div.querySelector('pre[data-md-raw]');
            if (!pre) { clearInterval(id); window.__pwStreamDone = true; return; }
            pre.textContent = text.slice(0, i++);
            if (i > text.length) { clearInterval(id); window.__pwStreamDone = true; }
        }, 20);
    });

    await page.waitForFunction(() => window.__pwStreamDone, { timeout: 10_000 });
    await page.waitForTimeout(500);

    const count = await page.evaluate(() => window.__pwRenderCount);
    expect(count).toBeLessThanOrEqual(12);
    expect(count).toBeGreaterThanOrEqual(1);
});

test('3. CDN failure falls back to plain text', async ({ page }) => {
    await page.evaluate(() => {
        document.body.classList.add('markdown-fallback');
        const div = document.createElement('div');
        div.id = 'pw-md-3';
        div.setAttribute('data-md', '1');
        div.innerHTML = '<pre data-md-raw># Still readable</pre>';
        document.body.appendChild(div);
    });

    await page.waitForTimeout(500);

    const pre = page.locator('#pw-md-3 pre[data-md-raw]');
    const text = await pre.textContent();
    expect(text).toBe('# Still readable');

    const bg = await pre.evaluate((el) => getComputedStyle(el).backgroundColor);
    // In fallback mode, the pre should have transparent background
    expect(bg).toBe('rgba(0, 0, 0, 0)');

    await page.evaluate(() => document.body.classList.remove('markdown-fallback'));
});

test('4. script tags are stripped from rendered output', async ({ page }) => {
    await page.waitForFunction(() => !!window.marked, { timeout: 10_000 });

    await page.evaluate(() => {
        const div = document.createElement('div');
        div.id = 'pw-md-4';
        div.setAttribute('data-md', '1');
        div.innerHTML = '<pre data-md-raw>Hello\n\n&lt;script&gt;alert(1)&lt;/script&gt;</pre>';
        document.body.appendChild(div);
    });

    await page.waitForTimeout(800);

    const html = await page.locator('#pw-md-4').innerHTML();
    expect(html).not.toContain('<script');
    expect(html).not.toContain('alert(1)');
});
