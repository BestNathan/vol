// Playwright e2e config for the React frontend (vol-llm-ui-react).
// Adapted from crates/vol-llm-ui/tests/web/playwright.config.js for the React
// dev server. Tests live in tests/e2e (excluded from vitest in vite.config.ts).
import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  expect: {
    // First load of the Vite dev server triggers dependency pre-bundling
    // which can take a while on cold starts; give assertions headroom.
    timeout: 15_000,
  },
  use: {
    baseURL: 'http://localhost:5173',
    headless: true,
    viewport: { width: 1280, height: 800 },
  },
  webServer: {
    command: 'npm run dev',
    port: 5173,
    reuseExistingServer: true,
    timeout: 60_000,
  },
})
