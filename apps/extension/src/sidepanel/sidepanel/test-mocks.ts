/**
 * `vi.mock` factories shared by the side-panel suites. Kept apart from
 * `test-support.ts` (which imports the mocked modules) so a suite's factory can
 * load this file without waiting on a module it is itself replacing.
 *
 * `sidepanel.ts` has no exports: every collaborator it mounts is replaced with
 * a recording stub, and the suites assert on what it handed them. Each suite
 * registers, e.g.:
 *   vi.mock('../../prep/prep', async () => (await import('./test-mocks')).prepMock());
 */

import { vi } from 'vitest';

import type * as JobToolsModule from '../../job-tools/job-tools';

export const PANEL_WINDOW_ID = 100;

/** What a `vi.mock` factory returns: the replacement module's exports. */
type MockModule = Record<string, unknown>;

const tabView = () => ({ render: vi.fn(), refresh: vi.fn(), reset: vi.fn() });

export const answerToolsMock = (): MockModule => ({
  mountAnswerTools: vi.fn(() => ({ render: vi.fn() })),
  copyText: vi.fn(),
});

export async function jobToolsMock(): Promise<MockModule> {
  const actual = await vi.importActual<typeof JobToolsModule>('../../job-tools/job-tools');
  return { ...actual, mountJobTools: vi.fn(() => ({ render: vi.fn(), checkPage: vi.fn() })) };
}

export const jobStatusMock = (): MockModule => ({
  mountJobStatus: vi.fn(() => ({ refresh: vi.fn(), reset: vi.fn() })),
});

export const documentsMock = (): MockModule => ({ mountDocuments: vi.fn(tabView) });

export const prepMock = (): MockModule => ({ mountPrep: vi.fn(tabView) });

export const siteMemoryMock = (): MockModule => ({
  mountFirstFillConfirm: vi.fn(() => ({ confirm: vi.fn(async () => true), cancel: vi.fn() })),
  getRememberedHosts: vi.fn(async () => []),
  rememberHost: vi.fn(async () => undefined),
  hostOf: vi.fn((url: string | null) => (url ? new URL(url).hostname : null)),
});

export const connectionStatusMock = (): MockModule => ({
  mountConnectionStatus: vi.fn(() => ({ start: vi.fn() })),
});

export const answerStateMock = (): MockModule => ({
  // Mirrors the REAL subscribeAnswerState's shape: it never delivers
  // synchronously (the real one is `readAnswerState(tabId).then(onState)`),
  // only on a later microtask — a caller that assumes a same-tick delivery
  // (the exact bug the follow() sequencing suite guards against) would see this
  // mock behave identically to the real thing.
  subscribeAnswerState: vi.fn((_tabId: number, onState: (state: unknown) => void) => {
    queueMicrotask(() => onState(null));
    return vi.fn();
  }),
});

export function panelBrowserMock(): { browser: Record<string, unknown> } {
  return {
    browser: {
      runtime: {
        sendMessage: vi.fn(),
        onMessage: { addListener: vi.fn() },
        openOptionsPage: vi.fn(),
      },
      // `local` IS present — `lib/theme.ts`'s `bootTheme()` reads it at load,
      // and `lib/appearance.ts`'s `getDefaultPanelTab()`. `session` defaults to
      // "nothing stored" for every test EXCEPT the lifecycle suite's
      // active-tab-restore block, which drives it — that default keeps
      // `sessionArea()` degrading the same way it did when this key was absent
      // altogether (mirrors `lib/answer-state.ts`'s own best-effort discipline).
      storage: {
        local: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
        session: {
          get: vi.fn(() => Promise.resolve({})),
          set: vi.fn(() => Promise.resolve(undefined)),
          // Resolves (never `undefined`) so the #1236 Half B cleanup's
          // `.catch(() => undefined)` chain is well-formed.
          remove: vi.fn(() => Promise.resolve(undefined)),
        },
        // `lib/theme.ts`'s `subscribeThemeChanges()` registers on `onChanged`
        // at panel load (#1236 Half A).
        onChanged: { addListener: vi.fn(), removeListener: vi.fn() },
      },
      windows: {
        getCurrent: vi.fn(() => Promise.resolve({ id: PANEL_WINDOW_ID })),
        onFocusChanged: { addListener: vi.fn() },
        onRemoved: { addListener: vi.fn() },
      },
      tabs: {
        query: vi.fn(({ windowId }: { windowId: number }) =>
          Promise.resolve(windowId === PANEL_WINDOW_ID ? [{ id: 7 }] : [])
        ),
        onActivated: { addListener: vi.fn() },
        create: vi.fn(),
      },
    },
  };
}
