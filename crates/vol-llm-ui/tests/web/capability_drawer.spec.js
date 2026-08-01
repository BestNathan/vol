// @ts-check
const { test, expect } = require('@playwright/test');

test.describe('Capability Drawer', () => {
    test('drawer is NOT visible on page load', async ({ page }) => {
        // Navigate to app (baseURL from playwright.config.js)
        await page.goto('/', { timeout: 30_000, waitUntil: 'domcontentloaded' });

        // Wait for the app to load (WS connected)
        await page.waitForSelector('text=Agents', { timeout: 15000 });

        // The ✎ button may be disabled without agent selection, so for now
        // verify the drawer does NOT cover the page on load.
        const drawer = page.locator('.fixed.right-0.top-0.h-full.w-80');
        await expect(drawer).not.toBeVisible();
    });
});
