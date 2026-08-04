// @ts-check
// Task 6.3 final integration smoke test — dark theme token verification plus
// the app-wide checklist:
//   1. StatusBar connection indicator + session info
//   2. All 7 tabs switch and render their panels
//   3. Agent cards render (mocked backend)
//   4. Conversation timeline renders (streamed content)
//   5. Capability drawer opens, search filters, toggles work
//   6. Dialogs open and close (DebugPanel, ApprovalDialog, shadcn SkillDetailDialog)
//   7. FileTree expands/collapses directories
//   8. Mobile responsive layout at 480px
//
// Runs against the Vite dev server with the WebSocket-level mock backend
// (helpers/mock-backend.js) — no live vol-agent-server needed.
import { test, expect } from '@playwright/test'
import { installMockBackend, selectAgent } from './helpers/mock-backend.js'

const RUN_ID = 'run-smoke'
let backend

function streamEvent(variant, data = {}) {
  return { run_id: RUN_ID, event: { [variant]: data } }
}

// Minimal workspace so the FileTree has a dir to expand/collapse. Entries must
// look like FileEntry (name/path/is_dir/children) — see FileTree.tsx.
const FILES = {
  '.': {
    entries: [
      { name: 'src', path: 'src', is_dir: true, children: [] },
      { name: 'README.md', path: 'README.md', is_dir: false, children: [] },
    ],
  },
  'src': {
    entries: [
      { name: 'main.ts', path: 'src/main.ts', is_dir: false, children: [] },
      { name: 'lib', path: 'src/lib', is_dir: true, children: [] },
    ],
  },
}

test.beforeEach(async ({ page }) => {
  backend = await installMockBackend(page, {
    handlers: {
      'file.list': (params) => FILES[params?.path] ?? { entries: [] },
      'file.read': () => ({ content: '# README\n\nmock content\n' }),
      'skill.list': () => ({
        skills: [{ id: 's1', name: 'test-skill', version: '1.0', scope: 'repo', description: 'Test skill', triggers: [] }],
      }),
      // Wire shape is { name, skill: SkillDetail } (see lib/protocol.ts).
      'skill.get': () => ({
        name: 'test-skill',
        skill: {
          name: 'test-skill', version: '1.0', scope: 'repo', description: 'Test skill',
          triggers: [], content: 'Test skill content', file_listing: [], directory: '.',
        },
      }),
    },
  })
})

test('1. dark theme tokens: body background resolves to #1a1a2e', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' })
  // --background: 240 28% 14% (was 240 10% 11%) so bg-background = #1a1a2e.
  const bg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor)
  expect(bg).toBe('rgb(26, 26, 46)')
})

test('2. status bar shows connection indicator and session info', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' })
  await expect(page.getByText('Connected')).toBeVisible()
  await expect(page.getByText('Session: web-sess')).toBeVisible()
  await expect(page.getByText(/Run: 0/)).toBeVisible()
  // Nodes dropdown reflects the mocked control-plane node.
  await expect(page.getByRole('button', { name: /Nodes\(1\)/ })).toBeVisible()
})

test('3. all 7 tabs switch and render their panels', async ({ page }) => {
  await selectAgent(page)
  const cases = [
    ['Tasks', 'No tasks found'],
    ['Agents', 'Test Agent'],
    ['Tools', 'No tools available'],
    ['Workspace', 'Click a file in the explorer to open it'],
    ['Skills', page.getByRole('cell', { name: 'test-skill' })], // mobile card + desktop table both contain the name; use the visible table cell
    ['MCP', 'No MCP servers configured'],
    ['Logs', 'No log files found.'],
  ]
  for (const [tab, marker] of cases) {
    await page.getByRole('button', { name: tab, exact: true }).click()
    const target = typeof marker === 'string' ? page.getByText(marker) : marker
    await expect(target.first()).toBeVisible()
  }
})

test('4. conversation timeline renders streamed content', async ({ page }) => {
  await selectAgent(page)
  backend.pushEvent(streamEvent('ContentStart'))
  backend.pushEvent(streamEvent('ContentDelta', { delta: 'Hello from the mock backend' }))
  backend.pushEvent(streamEvent('ContentComplete', { content: 'Hello from the mock backend' }))
  const prose = page.locator('.prose')
  await expect(prose).toBeVisible()
  await expect(prose).toContainText('Hello from the mock backend')
})

test('5. capability drawer opens, search filters, and toggles work', async ({ page }) => {
  await selectAgent(page)
  await page.getByRole('button', { name: 'Edit capabilities' }).click()
  const drawer = page.locator('div.fixed.right-0.top-0.h-full').filter({ hasText: 'Capabilities' })
  await expect(drawer).toBeVisible()

  // Search filters the available tools.
  await page.getByPlaceholder(/search/i).fill('read')
  await expect(page.getByRole('switch', { name: 'read_file' })).toBeVisible()
  await expect(page.getByRole('switch', { name: 'bash' })).toBeHidden()

  // Clear the filter and toggle a capability on.
  await page.getByPlaceholder(/search/i).fill('')
  const toggle = page.getByRole('switch', { name: 'bash' })
  await expect(toggle).toHaveAttribute('aria-checked', 'false')
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', 'true')
  await expect(page.locator('[aria-label="Saved bash"]')).toBeVisible()

  // Close via the header ✕.
  await page.getByRole('button', { name: 'Close capabilities drawer' }).click()
  await expect(drawer).not.toBeVisible()
})

test('6. dialogs open and close: DebugPanel, ApprovalDialog, shadcn SkillDetailDialog', async ({ page }) => {
  await selectAgent(page)

  // DebugPanel: toggled from the StatusBar bug button. While open its overlay
  // covers the whole screen, so it is closed via its own close button.
  await page.getByRole('button', { name: 'Toggle debug panel' }).click()
  await expect(page.getByText('Debug Panel')).toBeVisible()
  await page.getByRole('button', { name: 'Close debug panel' }).click()
  await expect(page.getByText('Debug Panel')).toBeHidden()

  // ApprovalDialog: pushed by an approval_request event.
  backend.pushEvent(streamEvent('ApprovalRequest', { tool_name: 'bash', reason: 'smoke', arguments: 'ls' }))
  await expect(page.getByRole('dialog', { name: 'Tool approval required' })).toBeVisible()
  await page.getByRole('button', { name: 'Approve' }).click()
  await expect(page.getByRole('dialog', { name: 'Tool approval required' })).toBeHidden()

  // shadcn Tabs: the Agents panel's Radix sub-tab bar switches panels. Must
  // run while an agent is still selected (sub-tabs render only then; switching
  // away from the Agents tab clears the selection).
  await page.getByRole('tab', { name: 'Sessions' }).click()
  await expect(page.getByText('No sessions found')).toBeVisible()
  await page.getByRole('tab', { name: 'Conversation' }).click()

  // shadcn Dialog: SkillDetailDialog from the Skills tab.
  await page.getByRole('button', { name: 'Skills', exact: true }).click()
  await page.getByRole('cell', { name: 'test-skill' }).click()
  const detail = page.getByRole('dialog').filter({ hasText: 'test-skill' })
  await expect(detail).toBeVisible()
  await detail.getByRole('button', { name: 'Close' }).click()
  await expect(detail).toBeHidden()
})

test('7. FileTree expands and collapses directories', async ({ page }) => {
  await selectAgent(page)
  const tree = page.locator('div[class*="bg-[#16162a]"]').first()
  const srcRow = tree.locator('span[class*="8ab4ff"]', { hasText: 'src' })
  await expect(srcRow).toBeVisible()

  // Expand: click the src dir -> its children load and render.
  await srcRow.click()
  const mainTs = tree.locator('span[class*="text-[#ccc]"]', { hasText: 'main.ts' })
  await expect(mainTs).toBeVisible()
  await expect(tree.locator('span[class*="8ab4ff"]', { hasText: 'lib' })).toBeVisible()

  // Collapse: click again -> children hidden.
  await srcRow.click()
  await expect(mainTs).toBeHidden()

  // Re-expand still works.
  await srcRow.click()
  await expect(mainTs).toBeVisible()
})

test('8. mobile <480px: rail FileTree, hidden StatusBar labels, drawer overlay', async ({ page }) => {
  // The `sm` breakpoint is min-width: 480px (Tailwind @theme override), so the
  // mobile layout is anything below 480px — 400px exercises it.
  await page.setViewportSize({ width: 400, height: 800 })
  await selectAgent(page)

  // FileTree collapsed to the 40px rail.
  await expect(page.getByRole('button', { name: 'Open file explorer' })).toBeVisible()
  // Session label hidden below sm.
  await expect(page.getByText('Session: web-sess')).toBeHidden()

  // Open the drawer overlay from the rail; dirs still expand inside it.
  await page.getByRole('button', { name: 'Open file explorer' }).click()
  await expect(page.getByRole('button', { name: 'Close file explorer' })).toBeVisible()
  const tree = page.locator('div[class*="bg-[#16162a]"]').first()
  await tree.locator('span[class*="8ab4ff"]', { hasText: 'src' }).click()
  await expect(tree.locator('span[class*="text-[#ccc]"]', { hasText: 'main.ts' })).toBeVisible()
  await page.getByRole('button', { name: 'Close file explorer' }).click()

  // Tabs still switch on a narrow screen (bar scrolls horizontally).
  await page.getByRole('button', { name: 'Logs', exact: true }).click()
  await expect(page.getByText('No log files found.')).toBeVisible()
})
