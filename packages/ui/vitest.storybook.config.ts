import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { storybookTest } from '@storybook/addon-vitest/vitest-plugin';
import { playwright } from '@vitest/browser-playwright';
import { defineConfig } from 'vitest/config';

const dirname = path.dirname(fileURLToPath(import.meta.url));

// Storybook browser-test project (`--project storybook`). `storybookTest`
// reuses .storybook/main.ts — including the viteFinal Tailwind plugin and the
// preview.css design-system import — so every story becomes a test that renders
// with the real styles, and `play` functions run as interaction tests. Executes
// headless Chromium via Playwright. Registered in the root vitest.config.ts
// `projects` array so it runs as part of the workspace test suite.
export default defineConfig({
  plugins: [storybookTest({ configDir: path.join(dirname, '.storybook') })],
  test: {
    name: 'storybook',
    setupFiles: [path.join(dirname, '.storybook/vitest.setup.ts')],
    browser: {
      enabled: true,
      headless: true,
      provider: playwright(),
      instances: [{ browser: 'chromium' }],
      // Vitest defaults this server to 63315 and allocates upward from there
      // for extra instances. Windows hands out Hyper-V/WSL port reservations in
      // that neighbourhood (a 63285-63384 block is typical), and a reserved
      // port is offered by the resolver but refused by bind — so the run dies
      // with `listen EACCES ::1:63315` while every test passes. Both the
      // default and this are fixed ports, so pinning low costs nothing and
      // sidesteps the whole dynamic-range exclusion class.
      api: { port: 6317 },
    },
  },
});
