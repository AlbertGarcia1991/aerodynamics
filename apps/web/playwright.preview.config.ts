import { defineConfig, devices } from '@playwright/test';

/**
 * Smoke tests against the **production build** (`dist/` via `vite preview`).
 * Catches what the dev server hides: asset paths of the worker and the .wasm,
 * minification issues, dev-only code leaking into the bundle.
 *   npx playwright test -c playwright.preview.config.ts
 */
export default defineConfig({
  testDir: './e2e',
  testMatch: /prod-smoke\.spec\.ts/,
  timeout: 60_000,
  reporter: [['list']],
  use: { baseURL: 'http://127.0.0.1:4173', viewport: { width: 1400, height: 900 } },
  webServer: {
    command: 'npm run build && npx vite preview --port 4173 --strictPort --host 127.0.0.1',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: false,
    timeout: 180_000,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
