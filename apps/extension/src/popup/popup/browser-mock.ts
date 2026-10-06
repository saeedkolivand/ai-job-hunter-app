/**
 * The `@wxt-dev/browser` stub the popup suites share. Kept apart from
 * `test-support.ts` (which imports the real `browser` binding) so a suite's
 * `vi.mock` factory can load it without waiting on the very module it is
 * replacing.
 */

import { vi } from 'vitest';

export function popupBrowserMock(): { browser: Record<string, unknown> } {
  return {
    browser: {
      runtime: {
        sendMessage: vi.fn(),
        onMessage: { addListener: vi.fn() },
        openOptionsPage: vi.fn(),
        getManifest: vi.fn(() => ({ version: '1.2.3' })),
      },
      // `query` resolves the tab id the shared answer state is keyed by (ADR-044)
      // — available without the `tabs` permission, which stays on the denylist.
      tabs: { create: vi.fn(), query: vi.fn(() => Promise.resolve([{ id: 7 }])) },
      storage: {
        session: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
        // `lib/theme.ts`'s `bootTheme()` reads this at module load.
        local: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
        onChanged: { addListener: vi.fn(), removeListener: vi.fn() },
      },
    },
  };
}
