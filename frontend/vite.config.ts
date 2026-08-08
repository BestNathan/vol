import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// `defineConfig` from `vitest/config` types the vitest-only `test` key (vite
// ignores it at runtime) — vitest reads it when it loads this config file.
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
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/components/ui/**', // shadcn/ui generated primitives
        'src/**/*.d.ts',
      ],
      // TODO(quality-gates): raise thresholds as test coverage improves.
      // Currently set to the measured baseline (2026-08-08): lines/stmts
      // 17.93%, funcs 47.23%, branches 80%. Prettier/lint/type gates run at
      // pre-commit; coverage is a pre-push gate.
      thresholds: {
        lines: 17,
        functions: 47,
        branches: 50,
        statements: 17,
      },
    },
  },
} satisfies import('vitest/config').UserConfig

export default defineConfig(config)
