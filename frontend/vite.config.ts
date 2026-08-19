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
    // Two-tier vitest split (Playwright e2e is a separate toolchain):
    //   unit        — pure-logic tests, node environment (tests/unit/)
    //   integration — component interaction tests, jsdom (tests/integration/)
    // Each project is selected via `--project <name>`; `vitest run` executes
    // both. `extends: true` inherits the root test config (exclude/coverage).
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
    projects: [
      {
        extends: true,
        test: {
          name: 'unit',
          environment: 'node',
          include: ['tests/unit/**/*.test.ts'],
        },
      },
      {
        extends: true,
        test: {
          name: 'integration',
          environment: 'jsdom',
          include: ['tests/integration/**/*.test.{ts,tsx}'],
          setupFiles: ['./tests/integration/setup.ts'],
        },
      },
    ],
  },
} satisfies import('vitest/config').UserConfig

export default defineConfig(config)
