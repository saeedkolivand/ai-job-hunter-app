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

import { describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('../answer-tools/answer-tools', () => ({
  mountAnswerTools: vi.fn(() => ({ render: vi.fn() })),
  copyText: vi.fn(),
}));

vi.mock('../job-tools/job-tools', () => ({
  mountJobTools: vi.fn(() => ({ render: vi.fn(), checkPage: vi.fn() })),
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
    runtime: { sendMessage: vi.fn(), onMessage: { addListener: vi.fn() } },
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

document.body.innerHTML =
  '<div id="view-connected" hidden></div>' +
  '<div id="job-tools-host"></div><div id="answer-tools-host"></div>' +
  '<div id="connection-pill-host"></div><div id="connection-views-host"></div>';

const { subscribeAnswerState } = await import('../lib/answer-state');
const { mountJobTools } = await import('../job-tools/job-tools');
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
