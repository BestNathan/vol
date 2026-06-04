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
