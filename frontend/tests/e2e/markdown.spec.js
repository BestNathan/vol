// @ts-check
// Markdown e2e tests — adapted from crates/vol-llm-ui/tests/web/markdown.spec.js
// for the React frontend.
//
// The React Markdown component (src/components/shared/Markdown.tsx) renders
// via react-markdown into a `.prose` container and throttles content updates
// to one render per 80ms (useThrottledValue) — max 12.5 renders/sec. Content
// is streamed into the conversation through mocked agent.event notifications
// (helpers/mock-backend.js), exercising the real React render pipeline.
//
// Removed vs. the Dioxus original: the CDN-failure fallback test — the React
// version bundles react-markdown, there is no CDN to fall back from.
import { test, expect } from '@playwright/test'
import { installMockBackend, selectAgent } from './helpers/mock-backend.js'

const RUN_ID = 'run-md'
// Populated in beforeEach; tests in a file run serially so module state is safe.
let backend

function streamEvent(variant, data = {}) {
  return { run_id: RUN_ID, event: { [variant]: data } }
}

/** Push a full markdown message: ContentStart, one delta, ContentComplete. */
function pushMarkdown(content) {
  backend.pushEvent(streamEvent('ContentStart'))
  backend.pushEvent(streamEvent('ContentDelta', { delta: content }))
  backend.pushEvent(streamEvent('ContentComplete', { content }))
}

test.beforeEach(async ({ page }) => {
  backend = await installMockBackend(page)
  await selectAgent(page)
})

test('1. static markdown renders correctly', async ({ page }) => {
  pushMarkdown('# Title\n\n**bold** and `code` and a list:\n- one\n- two')

  const prose = page.locator('.prose')
  await expect(prose).toBeVisible()
  await expect(prose.locator('h1')).toHaveText('Title')
  await expect(prose.locator('strong')).toHaveText('bold')
  await expect(prose.locator('code').first()).toHaveText('code')
  await expect(prose.locator('li').first()).toHaveText('one')
  await expect(prose.locator('li').nth(1)).toHaveText('two')
})

test('2. streaming render is throttled to <= 12 renders/sec', async ({ page }) => {
  backend.pushEvent(streamEvent('ContentStart'))
  const prose = page.locator('.prose')
  await expect(prose).toBeVisible()

  // Count distinct DOM commits of the markdown container: each throttled
  // state update produces exactly one new innerHTML snapshot.
  await page.evaluate(() => {
    const el = document.querySelector('.prose')
    window.__pwRenderCount = 0
    let last = null
    const mo = new MutationObserver(() => {
      const html = el.innerHTML
      if (html !== last) {
        last = html
        window.__pwRenderCount++
      }
    })
    mo.observe(el, { subtree: true, childList: true, characterData: true })
  })

  // Stream one char per 20ms (~660ms for the full text). With the 80ms
  // throttle this yields ~9-10 DOM commits — far fewer than the 34 updates.
  const text = 'Streaming markdown throttle test.'
  for (let i = 1; i <= text.length; i++) {
    backend.pushEvent(streamEvent('ContentDelta', { delta: text.slice(0, i) }))
    await page.waitForTimeout(20)
  }
  backend.pushEvent(streamEvent('ContentComplete', { content: text }))
  // Let the trailing throttle timer flush.
  await page.waitForTimeout(300)

  const count = await page.evaluate(() => window.__pwRenderCount)
  expect(count).toBeLessThanOrEqual(12)
  expect(count).toBeGreaterThanOrEqual(1)
})

test('3. script tags are stripped from rendered output', async ({ page }) => {
  pushMarkdown('Hello\n\n<script>alert(1)</script>')

  const prose = page.locator('.prose')
  await expect(prose).toBeVisible()
  await expect(prose).toContainText('Hello')

  const html = await prose.innerHTML()
  expect(html).not.toContain('<script')
  expect(html).not.toContain('alert(1)')
})
