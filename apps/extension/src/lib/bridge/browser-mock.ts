import { vi } from 'vitest';

/**
 * Stand-in for `@wxt-dev/browser` in the bridge tests (`vi.mock('@wxt-dev/browser',
 * () => import('./browser-mock'))`). `connectNative` THROWS by default so the ws
 * suites go straight to the port probe — native is treated as "host unavailable
 * → fall back to ws"; the native suite overrides it per-test.
 */
export const browser = {
  runtime: {
    connectNative: vi.fn(() => {
      throw new Error('connectNative not available');
    }),
    lastError: undefined,
  },
};
