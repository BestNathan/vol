// @ts-check
// Capability drawer e2e tests — adapted from
// crates/vol-llm-ui/tests/web/capability_drawer.spec.js for the React
// frontend. Selectors target the React DOM (shadcn-style utility classes and
// aria-labels) instead of Dioxus-generated attributes. The JSON-RPC backend
// is mocked at the WebSocket layer (helpers/mock-backend.js) so the tests run
// against the Vite dev server without a live vol-agent-server.
import { test, expect } from '@playwright/test'
import { installMockBackend, selectAgent } from './helpers/mock-backend.js'

// The drawer panel is a fixed right-side container holding the "Capabilities"
// header (src/components/inputs/CapabilityDrawer.tsx). It returns null from
// the React tree while closed, so this locator matches nothing on load.
const drawerPanel = (page) =>
  page.locator('div.fixed.right-0.top-0.h-full').filter({ hasText: 'Capabilities' })

test.beforeEach(async ({ page }) => {
  await installMockBackend(page)
})

test('drawer is NOT visible on page load', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' })
  await expect(drawerPanel(page)).not.toBeVisible()
})

test('drawer opens when the edit-capabilities button is clicked and closes via ✕', async ({ page }) => {
  await selectAgent(page)
  await page.getByRole('button', { name: 'Edit capabilities' }).click()
  await expect(drawerPanel(page)).toBeVisible()
  // Close button in the drawer header.
  await page.getByRole('button', { name: 'Close capabilities drawer' }).click()
  await expect(drawerPanel(page)).not.toBeVisible()
})

test('capability toggle flips the switch and shows saved feedback', async ({ page }) => {
  await selectAgent(page)
  await page.getByRole('button', { name: 'Edit capabilities' }).click()

  const toggle = page.getByRole('switch', { name: 'bash' })
  await expect(toggle).toBeVisible()
  await expect(toggle).toHaveAttribute('aria-checked', 'false')

  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', 'true')
  // Mock agent.update_capabilities resolves -> optimistic toggle is confirmed
  // with a transient checkmark (aria-label "Saved <name>", ages out after 1.5s).
  await expect(page.locator('[aria-label="Saved bash"]')).toBeVisible()

  // Drawer stays open after toggling.
  await expect(drawerPanel(page)).toBeVisible()
})
