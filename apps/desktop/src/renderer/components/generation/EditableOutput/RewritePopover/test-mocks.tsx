/**
 * Mock factories shared by the RewritePopover tests. `vi.mock` is hoisted per
 * test file, so each file wires these in with
 * `vi.mock('<pkg>', async () => (await import('./test-mocks')).<factory>())`.
 * Kept free of any import of the component so a factory never awaits a module
 * that itself imports the mocked package.
 */
import { vi } from 'vitest';

import type * as AjhUi from '@ajh/ui';

/** The bound `resolveRewriteTimeoutMs` hands back in these tests — deliberately
 *  far above the deleted 60 s constant, so a test advancing past 60 s can prove
 *  the popover no longer aborts there. */
export const RESOLVED_TIMEOUT_MS = 300_000;

export const translationsMock = () => ({
  useTranslation: () => ({ t: (k: string) => k }),
});

/** `rewriteSelection` stalls by default (controlled per-test via mockImplementation). */
export const generateMock = (): Record<string, unknown> => ({
  rewriteSelection: vi.fn(),
  resolveRewriteTimeoutMs: () => RESOLVED_TIMEOUT_MS,
});

/** Strip animation props, render a plain div. */
export const motionMock = () => ({
  motion: {
    div: ({
      initial: _i,
      animate: _a,
      exit: _e,
      transition: _t,
      ...rest
    }: React.HTMLAttributes<HTMLDivElement> & {
      initial?: unknown;
      animate?: unknown;
      exit?: unknown;
      transition?: unknown;
    }) => <div {...rest} />,
  },
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
});

/** The real `@ajh/ui` with the focus trap stubbed (a ref object the mock div accepts as a plain prop). */
export const uiMock = (actual: typeof AjhUi): Record<string, unknown> => ({
  ...actual,
  useFocusTrap: () => ({ current: null }),
});
