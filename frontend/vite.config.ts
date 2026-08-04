import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// `test` is a vitest-only key (vite ignores it at runtime); declared here so
// tsc accepts it — vitest reads it when it loads this config file.
const config = {
  plugins: [react(), tailwindcss()],
  define: {
    __BUILD_TIME__: JSON.stringify(new Date().toISOString()),
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/ws': {
        target: 'ws://localhost:3001',
        ws: true,
      },
    },
  },
  test: {
    // Playwright e2e specs live in tests/e2e and must not be picked up by
    // vitest (they import @playwright/test and need a browser). Note: a user
    // `exclude` REPLACES vitest's defaults (deepMerge array semantics), so the
    // default excludes are repeated here.
    exclude: [
      'tests/e2e/**',
      '**/node_modules/**',
      '**/dist/**',
      '**/cypress/**',
      '**/.{idea,git,cache,output,temp}/**',
      '**/{karma,rollup,webpack,vite,vitest,jest,ava,babel,nyc,cypress,tsup,build,eslint,prettier}.config.*',
    ],
  },
} satisfies import('vite').UserConfig & { test: { exclude: string[] } }

export default defineConfig(config)
