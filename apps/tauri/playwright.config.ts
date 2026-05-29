import { defineConfig, devices } from '@playwright/test';

const PORT = 5174;

/**
 * E2E config — drives the renderer in a real browser against the in-memory
 * mock client (see src/e2e-main.tsx + e2e.html). The Vite dev server serves
 * the e2e entry; no Tauri/native build is required.
 */
export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: `http://localhost:${PORT}/e2e.html`,
    trace: 'on-first-retry',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'pnpm dev:frontend',
    url: `http://localhost:${PORT}/e2e.html`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
