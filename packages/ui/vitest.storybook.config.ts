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
    },
  },
});
