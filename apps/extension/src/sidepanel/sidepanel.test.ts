/**
 * Unit tests for the side panel's window-scoping (sidepanel.ts).
 *
 * The panel is per WINDOW: it resolves its own window once at load and must
 * ignore any `tabs.onActivated` activation that fires in a DIFFERENT window
 * (a tab switch in an unrelated browser window must never hijack this
 * panel's subscription — CodeRabbit finding, PR #1108). `mountAnswerTools`
 * and `subscribeAnswerState` are mocked so these tests exercise only the
 * window-scoping decision, not rendering.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type * as JobToolsModule from '../job-tools/job-tools';

vi.mock('../answer-tools/answer-tools', () => ({
  mountAnswerTools: vi.fn(() => ({ render: vi.fn() })),
  copyText: vi.fn(),
}));

vi.mock('../job-tools/job-tools', async () => {
  const actual = await vi.importActual<typeof JobToolsModule>('../job-tools/job-tools');
  return {
    ...actual,
    mountJobTools: vi.fn(() => ({ render: vi.fn(), checkPage: vi.fn() })),
  };
});

vi.mock('../job-status/job-status', () => ({
  mountJobStatus: vi.fn(() => ({ refresh: vi.fn(), reset: vi.fn() })),
}));

vi.mock('../lib/site-memory', () => ({
  mountFirstFillConfirm: vi.fn(() => ({ confirm: vi.fn(async () => true), cancel: vi.fn() })),
  getRememberedHosts: vi.fn(async () => []),
  rememberHost: vi.fn(async () => undefined),
  hostOf: vi.fn((url: string | null) => (url ? new URL(url).hostname : null)),
}));

vi.mock('../connection-status/connection-status', () => ({
  mountConnectionStatus: vi.fn(() => ({ start: vi.fn() })),
}));

vi.mock('../lib/answer-state', () => ({
  // Mirrors the REAL subscribeAnswerState's shape: it never delivers
  // synchronously (the real one is `readAnswerState(tabId).then(onState)`),
  // only on a later microtask — a caller that assumes a same-tick delivery
  // (the exact bug this file's "follow() sequencing" describe block guards
  // against) would see this mock behave identically to the real thing.
  subscribeAnswerState: vi.fn((_tabId: number, onState: (state: unknown) => void) => {
    queueMicrotask(() => onState(null));
    return vi.fn();
  }),
}));

const PANEL_WINDOW_ID = 100;

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    runtime: {
      sendMessage: vi.fn(),
      onMessage: { addListener: vi.fn() },
      openOptionsPage: vi.fn(),
    },
    // `local` IS present — `lib/theme.ts`'s `bootTheme()` reads it at load,
    // and (below) `lib/appearance.ts`'s `getDefaultPanelTab()`. `session`
    // defaults to "nothing stored" for every test EXCEPT the dedicated
    // "active tab restore at load" describe block, which drives it — that
    // default keeps `sessionArea()` degrading the same way it did when this
    // key was absent altogether (mirrors `lib/answer-state.ts`'s own
    // best-effort discipline for the same gap).
    storage: {
      local: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
      session: {
        get: vi.fn(() => Promise.resolve({})),
        set: vi.fn(() => Promise.resolve(undefined)),
      },
    },
    windows: {
      getCurrent: vi.fn(() => Promise.resolve({ id: PANEL_WINDOW_ID })),
      onFocusChanged: { addListener: vi.fn() },
    },
    tabs: {
      query: vi.fn(({ windowId }: { windowId: number }) =>
        Promise.resolve(windowId === PANEL_WINDOW_ID ? [{ id: 7 }] : [])
      ),
      onActivated: { addListener: vi.fn() },
    },
  },
}));

/** The DOM `sidepanel.ts` queries via `byId` at module load — factored out so
 *  the "active tab restore at load" tests (below) can rebuild it fresh before
 *  each `vi.resetModules()` + reimport. */
function buildPanelDom(): void {
  document.body.innerHTML =
    '<header><h1 class="title">AI Job Hunter</h1>' +
    '<div id="connection-pill-host"><button id="btn-settings"></button></div></header>' +
    '<section id="view-connected" hidden>' +
    '<p id="trust-line" hidden></p>' +
    '<div id="tabs-host"></div>' +
    '</section>' +
    '<div id="connection-views-host"></div>';
}

buildPanelDom();

// `sidepanel.ts` has no exports: everything under test — the `tabs.onActivated`
// listener, the `mountJobTools`/`mountConnectionStatus` calls and the deps they
// were handed — is recorded on these mocks by the import BELOW, once, before any
// test body runs. Vitest 5 turned `clearMocks` on by default (a
// `vi.clearAllMocks()` before every test), which wipes exactly that history —
// the migration guide names module-load recording as the most affected pattern
// (https://vitest.dev/guide/migration#clearmocks-is-enabled-by-default). Opt
// this file out; the runner restores the config after the file, so no other test
// file is affected, and the tests below still clear per-test history explicitly
// where they depend on it (`mockClear()`).
vi.setConfig({ clearMocks: false });

const { subscribeAnswerState } = await import('../lib/answer-state');
const { mountJobTools } = await import('../job-tools/job-tools');
const { mountJobStatus } = await import('../job-status/job-status');
const { mountFirstFillConfirm } = await import('../lib/site-memory');
const { mountConnectionStatus } = await import('../connection-status/connection-status');
await import('./sidepanel');

/** Flush the module-load `resolvePanelWindowId().then(...)` chain. */
function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe('sidepanel window scoping', () => {
  it('follows the active tab of its OWN window on load', async () => {
    await flush();
    expect(subscribeAnswerState).toHaveBeenCalledWith(7, expect.any(Function));
  });

  it('ignores a tabs.onActivated activation from a DIFFERENT window (regression)', async () => {
    await flush();
    vi.mocked(subscribeAnswerState).mockClear();

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');

    onActivated({ tabId: 55, windowId: 999 } as never);
    await flush();

    expect(subscribeAnswerState).not.toHaveBeenCalled();
  });

  it('follows an activation in its OWN window', async () => {
    await flush();
    vi.mocked(subscribeAnswerState).mockClear();

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');

    onActivated({ tabId: 42, windowId: PANEL_WINDOW_ID } as never);
    await flush();

    expect(subscribeAnswerState).toHaveBeenCalledWith(42, expect.any(Function));
  });
});

// ── connection-status composition (ADR-046) ─────────────────────────────────
// The panel's ONLY connection-status responsibility: show `#view-connected`
// (the job/answer tools) only while `phase === 'connected'`. The pill/retry/
// pairing/offline/outdated/searching behavior itself lives in
// `connection-status.ts` — see `connection-status.test.ts` for that.

describe('connection-status composition', () => {
  it('mounts against the pill + views hosts and starts it', () => {
    expect(vi.mocked(mountConnectionStatus)).toHaveBeenCalledWith(
      document.getElementById('connection-pill-host'),
      document.getElementById('connection-views-host'),
      expect.objectContaining({ send: expect.any(Function), onStatus: expect.any(Function) })
    );
    const view = vi.mocked(mountConnectionStatus).mock.results[0]?.value as
      { start: ReturnType<typeof vi.fn> } | undefined;
    expect(view?.start).toHaveBeenCalledTimes(1);
  });

  it('shows #view-connected only while phase === connected', () => {
    const onStatus = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onStatus;
    if (!onStatus) throw new Error('onStatus dep not captured');
    const viewConnected = document.getElementById('view-connected') as HTMLElement;

    onStatus({ phase: 'connected', port: 1, hasToken: true });
    expect(viewConnected.hidden).toBe(false);

    onStatus({ phase: 'app_not_running', port: null, hasToken: true });
    expect(viewConnected.hidden).toBe(true);

    onStatus({ phase: 'searching', port: null, hasToken: false });
    expect(viewConnected.hidden).toBe(true);
  });
});

// ── job-tools wiring (panel parity) ─────────────────────────────────────────
// `follow()` is BOTH of job-tools's own documented panel trigger points ("on
// mount and on tab activation" — see job-tools.ts's `JobToolsView.checkPage`
// doc): the initial call from `resolvePanelWindowId().then(...)` is the
// mount, every later call from `tabs.onActivated`/a focus change is an
// activation.

describe('job-tools wiring (panel parity)', () => {
  const jobTools = vi.mocked(mountJobTools).mock.results[0]?.value as
    { render: ReturnType<typeof vi.fn>; checkPage: ReturnType<typeof vi.fn> } | undefined;
  if (!jobTools) throw new Error('mountJobTools was not called at module load');

  it('mounts against #job-tools-host', () => {
    expect(vi.mocked(mountJobTools)).toHaveBeenCalledWith(
      document.getElementById('job-tools-host'),
      expect.objectContaining({ send: expect.any(Function) })
    );
  });

  it('calls checkPage again on every tab activation, not just at mount', async () => {
    await flush();
    const callsAfterMount = jobTools.checkPage.mock.calls.length;
    expect(callsAfterMount).toBeGreaterThan(0);

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 42, windowId: PANEL_WINDOW_ID } as never);
    await flush();

    expect(jobTools.checkPage.mock.calls.length).toBeGreaterThan(callsAfterMount);
  });

  it('feeds the subscribed AnswerState to jobTools.render alongside answerTools.render', async () => {
    vi.mocked(subscribeAnswerState).mockClear();
    jobTools.render.mockClear();

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 42, windowId: PANEL_WINDOW_ID } as never);
    await flush();

    const stateCallback = vi.mocked(subscribeAnswerState).mock.calls[0]?.[1];
    if (!stateCallback) throw new Error('subscribeAnswerState callback not captured');
    const fakeState = {
      tabId: 42,
      origin: 'https://jobs.example.com',
      scannedAt: 1,
      rows: [],
      stream: null,
      pageChanged: false,
    };
    stateCallback(fakeState as never);

    expect(jobTools.render).toHaveBeenCalledWith(fakeState);
  });
});

// ── follow() sequencing regression ───────────────────────────────────────────
// `checkPage()` must never run against a tab job-tools has not actually
// evaluated: `subscribeAnswerState`'s first delivery is unavoidably
// asynchronous, so a `checkPage()` call placed as a separate statement right
// after subscribing would fire against whatever trust was left over from the
// PREVIOUS tab (or the cold-mount default) — never the newly-followed tab.
// These tests drive the REAL order of operations (mount → subscribe → first
// async delivery) instead of calling `checkPage()`/`render()` in isolation,
// which is what let this regression ship uncaught the first time.

describe('follow() sequencing regression (checkPage must not race the async state delivery)', () => {
  const jobTools = vi.mocked(mountJobTools).mock.results[0]?.value as
    { render: ReturnType<typeof vi.fn>; checkPage: ReturnType<typeof vi.fn> } | undefined;
  if (!jobTools) throw new Error('mountJobTools was not called at module load');

  it("does not call checkPage before the tab's own state has actually been delivered", () => {
    let deliver: ((state: unknown) => void) | undefined;
    vi.mocked(subscribeAnswerState).mockImplementationOnce((_tabId, onState) => {
      deliver = onState as (state: unknown) => void;
      return vi.fn();
    });
    jobTools.checkPage.mockClear();
    jobTools.render.mockClear();

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 99, windowId: PANEL_WINDOW_ID } as never);

    // Nothing has been delivered yet — checkPage must not have fired against
    // whatever trust the PREVIOUS tab (or the cold-mount default) left behind.
    expect(jobTools.checkPage).not.toHaveBeenCalled();
    expect(jobTools.render).not.toHaveBeenCalled();

    // The async delivery lands. A non-optional call — if `deliver` was never
    // captured (the subscription didn't fire the way this test assumes),
    // this must fail loudly here, not silently no-op into a confusing
    // "expected N calls, got 0" a few lines down.
    if (!deliver) throw new Error('subscribeAnswerState callback not captured');
    deliver({
      tabId: 99,
      origin: 'https://jobs.example.com',
      scannedAt: 1,
      rows: [],
      stream: null,
      pageChanged: false,
    });

    // render() must run BEFORE checkPage() reads the trust it just set.
    expect(jobTools.render).toHaveBeenCalledTimes(1);
    expect(jobTools.checkPage).toHaveBeenCalledTimes(1);
    const renderOrder = jobTools.render.mock.invocationCallOrder[0];
    const checkPageOrder = jobTools.checkPage.mock.invocationCallOrder[0];
    expect(renderOrder).toBeLessThan(checkPageOrder as number);
  });

  it('fires checkPage only on the FIRST delivery for a followed tab, not on later pushes for the same tab', () => {
    let deliver: ((state: unknown) => void) | undefined;
    vi.mocked(subscribeAnswerState).mockImplementationOnce((_tabId, onState) => {
      deliver = onState as (state: unknown) => void;
      return vi.fn();
    });
    jobTools.checkPage.mockClear();

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 101, windowId: PANEL_WINDOW_ID } as never);

    const state = {
      tabId: 101,
      origin: 'https://jobs.example.com',
      scannedAt: 1,
      rows: [],
      stream: null,
      pageChanged: false,
    };
    if (!deliver) throw new Error('subscribeAnswerState callback not captured');
    deliver(state); // first delivery — checkPage fires
    deliver({ ...state, scannedAt: 2 }); // a later push (e.g. an answer accepted)

    expect(jobTools.checkPage).toHaveBeenCalledTimes(1);
  });

  it('ignores a stale follow(A) callback that resolves after follow(B) has already superseded it', () => {
    let deliverA: ((state: unknown) => void) | undefined;
    let deliverB: ((state: unknown) => void) | undefined;
    vi.mocked(subscribeAnswerState)
      .mockImplementationOnce((_tabId, onState) => {
        deliverA = onState as (state: unknown) => void;
        return vi.fn();
      })
      .mockImplementationOnce((_tabId, onState) => {
        deliverB = onState as (state: unknown) => void;
        return vi.fn();
      });
    jobTools.checkPage.mockClear();
    jobTools.render.mockClear();

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');

    // follow(A) — its read is kicked off but not yet resolved.
    onActivated({ tabId: 201, windowId: PANEL_WINDOW_ID } as never);
    // follow(B) supersedes A before A's read resolves. `subscribeAnswerState`'s
    // returned unsubscribe (called here) does NOT cancel A's in-flight read —
    // see `followGeneration`'s doc in sidepanel.ts for why that matters.
    onActivated({ tabId: 202, windowId: PANEL_WINDOW_ID } as never);

    if (!deliverB) throw new Error('deliverB not captured');
    deliverB({
      tabId: 202,
      origin: 'https://jobs.example.com',
      scannedAt: 1,
      rows: [],
      stream: null,
      pageChanged: false,
    });
    jobTools.render.mockClear();
    jobTools.checkPage.mockClear();

    // A's stale read finally resolves — it must be a complete no-op.
    if (!deliverA) throw new Error('deliverA not captured');
    deliverA({
      tabId: 201,
      origin: 'https://jobs.example.com',
      scannedAt: 1,
      rows: [],
      stream: null,
      pageChanged: false,
    });

    expect(jobTools.render).not.toHaveBeenCalled();
    expect(jobTools.checkPage).not.toHaveBeenCalled();
  });
});

// ── PR0 §3: the gear button, the tab bar, the trust line, job-status ────────

describe('the gear button opens the Settings page', () => {
  it('calls browser.runtime.openOptionsPage on click', () => {
    document.getElementById('btn-settings')!.dispatchEvent(new Event('click', { bubbles: true }));
    expect(browser.runtime.openOptionsPage).toHaveBeenCalled();
  });
});

describe('the tab bar (Job / Answers)', () => {
  it('mounts two tabs, Job active by default', () => {
    const buttons = document.querySelectorAll<HTMLButtonElement>('.tab');
    expect(buttons).toHaveLength(2);
    expect(document.querySelector<HTMLButtonElement>('[data-tab="job"]')!.classList).toContain(
      'active'
    );
  });

  it('mounts job-status + job-tools into the Job panel, and answer-tools into the Answers panel', () => {
    const jobPanel = document.querySelector<HTMLElement>('[data-section="job"]')!;
    const answersPanel = document.querySelector<HTMLElement>('[data-section="answers"]')!;
    expect(jobPanel.querySelector('#job-tools-host')).not.toBeNull();
    expect(answersPanel.querySelector('#answer-tools-host')).not.toBeNull();
  });
});

describe('the trust line (ADR-045)', () => {
  it('shows "Reading: <host>" once a trusted state is delivered', async () => {
    const trustLine = document.getElementById('trust-line') as HTMLElement;
    vi.mocked(subscribeAnswerState).mockImplementationOnce((_tabId, onState) => {
      queueMicrotask(() =>
        onState({
          tabId: 42,
          origin: 'https://jobs.example.com',
          scannedAt: 1,
          rows: [],
          stream: null,
          pageChanged: false,
        } as never)
      );
      return vi.fn();
    });

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 42, windowId: PANEL_WINDOW_ID } as never);
    await new Promise((r) => setTimeout(r, 0));

    expect(trustLine.hidden).toBe(false);
    expect(trustLine.textContent).toBe('Reading: jobs.example.com');
  });

  it('hides again for an untrusted (pageChanged) state, leaving the message to job-tools', async () => {
    const trustLine = document.getElementById('trust-line') as HTMLElement;
    vi.mocked(subscribeAnswerState).mockImplementationOnce((_tabId, onState) => {
      queueMicrotask(() =>
        onState({
          tabId: 43,
          origin: 'https://jobs.example.com',
          scannedAt: 1,
          rows: [],
          stream: null,
          pageChanged: true,
        } as never)
      );
      return vi.fn();
    });

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 43, windowId: PANEL_WINDOW_ID } as never);
    await new Promise((r) => setTimeout(r, 0));

    expect(trustLine.hidden).toBe(true);
  });
});

describe('the Answers tab count badge', () => {
  it('reflects the delivered state row count', async () => {
    vi.mocked(subscribeAnswerState).mockImplementationOnce((_tabId, onState) => {
      queueMicrotask(() =>
        onState({
          tabId: 44,
          origin: 'https://jobs.example.com',
          scannedAt: 1,
          rows: [
            { id: 'a', question: 'Q', field: null, status: 'empty', versions: [], selected: -1 },
          ],
          stream: null,
          pageChanged: false,
        } as never)
      );
      return vi.fn();
    });

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 44, windowId: PANEL_WINDOW_ID } as never);
    await new Promise((r) => setTimeout(r, 0));

    expect(document.querySelector('[data-tab="answers"]')!.textContent).toBe('Answers (1)');
  });
});

// ── Fill confirmation binding (item 12): capture + revalidate ──────────────
// `mountJobTools` is mocked out entirely in this file (no real DOM/buttons),
// so these tests drive the REAL `confirmFill` closure sidepanel.ts built and
// handed to it — captured off `mountJobTools`'s own `mock.calls`, since the
// mock function itself ignores its arguments but still records them.

describe('Fill confirmation binding (item 12)', () => {
  it("follow() cancels whatever confirmation was open for the tab it's leaving", async () => {
    await flush();
    const fillConfirm = vi.mocked(mountFirstFillConfirm).mock.results[0]?.value;
    if (!fillConfirm) throw new Error('mountFirstFillConfirm was not called at module load');
    vi.mocked(fillConfirm.cancel).mockClear();

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 501, windowId: PANEL_WINDOW_ID } as never);

    expect(fillConfirm.cancel).toHaveBeenCalledTimes(1);
  });

  it('re-validates the captured origin/generation after confirm() resolves, aborting a stale confirmation even when the user answered Fill', async () => {
    await flush();
    const jobToolsCall = vi.mocked(mountJobTools).mock.calls[0] as unknown as [
      HTMLElement,
      JobToolsModule.JobToolsDeps,
    ];
    const confirmFill = jobToolsCall[1].confirmFill;
    if (!confirmFill) throw new Error('confirmFill dep not passed to mountJobTools');
    const fillConfirm = vi.mocked(mountFirstFillConfirm).mock.results[0]?.value;
    if (!fillConfirm) throw new Error('mountFirstFillConfirm was not called at module load');

    let resolveConfirm: ((v: boolean) => void) | undefined;
    vi.mocked(fillConfirm.confirm).mockReturnValueOnce(
      new Promise<boolean>((resolve) => {
        resolveConfirm = resolve;
      })
    );

    const pending = confirmFill();

    // The panel follows a DIFFERENT tab while the confirmation is still open
    // (bumps the follow generation and, in real usage, calls cancel() too —
    // this test's own assertion is about sidepanel.ts's re-validation, which
    // must hold regardless of whether the mocked confirm() ever "hears" it).
    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 502, windowId: PANEL_WINDOW_ID } as never);
    await flush();

    // The user eventually answers "Fill" on the now-stale confirmation.
    resolveConfirm?.(true);

    await expect(pending).resolves.toBe(false);
  });
});

// ── job-status refresh keyed by (origin, pageChanged) transitions (item 13) ─

describe('job-status refresh is keyed by transition, not every state push (item 13)', () => {
  it('refreshes once for N streamed updates that only change unrelated fields', async () => {
    const jobStatus = vi.mocked(mountJobStatus).mock.results[0]?.value;
    if (!jobStatus) throw new Error('mountJobStatus was not called at module load');
    vi.mocked(jobStatus.refresh).mockClear();

    let deliver: ((state: unknown) => void) | undefined;
    vi.mocked(subscribeAnswerState).mockImplementationOnce((_tabId, onState) => {
      deliver = onState as (state: unknown) => void;
      return vi.fn();
    });

    const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
    if (!onActivated) throw new Error('tabs.onActivated listener not registered');
    onActivated({ tabId: 601, windowId: PANEL_WINDOW_ID } as never);

    if (!deliver) throw new Error('subscribeAnswerState callback not captured');
    const base = {
      tabId: 601,
      origin: 'https://jobs.example.com',
      scannedAt: 1,
      rows: [],
      stream: null,
      pageChanged: false,
    };
    deliver(base); // first delivery for this tab
    deliver({ ...base, scannedAt: 2 }); // streamed chunk 1
    deliver({ ...base, scannedAt: 3 }); // streamed chunk 2
    deliver({ ...base, scannedAt: 4 }); // streamed chunk 3

    expect(jobStatus.refresh).toHaveBeenCalledTimes(1);
  });
});

// ── active tab restore at load — storage.session + Appearance default ──────
// (items 6 & 11). `loadActiveTab()` only runs once, at module load, so these
// drive a FRESH `sidepanel.ts` instance per test via `vi.resetModules()` —
// the mocked `@wxt-dev/browser` module itself is NOT re-evaluated by that
// (its `vi.fn()`s and any `mockResolvedValueOnce` queued below survive), only
// `sidepanel.ts` (and its other, real, non-mocked dependencies) are.

describe('active tab restore at load (storage.session + Appearance default, items 6 & 11)', () => {
  afterEach(() => {
    vi.mocked(browser.storage.session.get).mockReset().mockResolvedValue({});
    vi.mocked(browser.storage.session.set).mockReset().mockResolvedValue(undefined);
    vi.mocked(browser.storage.local.get).mockReset().mockResolvedValue({});
  });

  it('restores the tab stored in storage.session for this window', async () => {
    vi.mocked(browser.storage.session.get).mockResolvedValueOnce({
      'sidepanelActiveTab:100': 'answers',
    });
    vi.resetModules();
    buildPanelDom();

    await import('./sidepanel');
    await flush();

    expect(document.querySelector('[data-tab="answers"]')!.classList.contains('active')).toBe(true);
  });

  it('clicking Answers persists sidepanelActiveTab:<windowId> to storage.session', async () => {
    vi.resetModules();
    buildPanelDom();
    await import('./sidepanel');
    await flush();

    document.querySelector<HTMLButtonElement>('[data-tab="answers"]')!.click();

    expect(browser.storage.session.set).toHaveBeenCalledWith({
      'sidepanelActiveTab:100': 'answers',
    });
  });

  it('falls back to the Appearance default panel tab when nothing is stored for this window', async () => {
    // Call #1 to `local.get` is bootTheme()'s getTheme() (key 'theme',
    // synchronous at the top of the module); call #2 is
    // getDefaultPanelTab()'s own read (key 'defaultPanelTab', only once
    // resolvePanelWindowId()'s chain reaches loadActiveTab()) — a fixed
    // order, so queuing two values in sequence targets each correctly.
    vi.mocked(browser.storage.local.get)
      .mockResolvedValueOnce({})
      .mockResolvedValueOnce({ defaultPanelTab: 'answers' });
    vi.resetModules();
    buildPanelDom();

    await import('./sidepanel');
    await flush();

    expect(document.querySelector('[data-tab="answers"]')!.classList.contains('active')).toBe(true);
  });

  it('falls back to job without throwing when storage.session.get rejects', async () => {
    vi.mocked(browser.storage.session.get).mockRejectedValueOnce(new Error('quota'));
    vi.resetModules();
    buildPanelDom();

    await expect(import('./sidepanel')).resolves.toBeDefined();
    await flush();

    expect(document.querySelector('[data-tab="job"]')!.classList.contains('active')).toBe(true);
  });
});
