/**
 * Unit tests for how the side panel FOLLOWS a tab (sidepanel.ts `follow()`):
 * window scoping, the job-tools / documents / prep / job-status wiring it drives,
 * and the sequencing guarantees around the asynchronous state delivery.
 *
 * The panel is per WINDOW: it resolves its own window once at load and must
 * ignore any `tabs.onActivated` activation that fires in a DIFFERENT window
 * (a tab switch in an unrelated browser window must never hijack this
 * panel's subscription — CodeRabbit finding, PR #1108). Every collaborator is a
 * recording stub (see `test-mocks.ts`), so these tests exercise only the
 * panel's own decisions, not rendering.
 */

import { describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('../../answer-tools/answer-tools', () =>
  import('./test-mocks').then((m) => m.answerToolsMock())
);
vi.mock('../../job-tools/job-tools', () => import('./test-mocks').then((m) => m.jobToolsMock()));
vi.mock('../../job-status/job-status', () => import('./test-mocks').then((m) => m.jobStatusMock()));
vi.mock('../../documents/documents', () => import('./test-mocks').then((m) => m.documentsMock()));
vi.mock('../../prep/prep', () => import('./test-mocks').then((m) => m.prepMock()));
vi.mock('../../lib/site-memory', () => import('./test-mocks').then((m) => m.siteMemoryMock()));
vi.mock('../../connection-status/connection-status', () =>
  import('./test-mocks').then((m) => m.connectionStatusMock())
);
vi.mock('../../lib/answer-state', () => import('./test-mocks').then((m) => m.answerStateMock()));
vi.mock('@wxt-dev/browser', () => import('./test-mocks').then((m) => m.panelBrowserMock()));

import { mountDocuments } from '../../documents/documents';
import { mountJobStatus } from '../../job-status/job-status';
import { JOB_TOOLS_GATED_LINE, mountJobTools } from '../../job-tools/job-tools';
import { subscribeAnswerState } from '../../lib/answer-state';
import { mountPrep } from '../../prep/prep';
import {
  activate,
  bootPanel,
  captureNextSubscription,
  deliverOnNextSubscription,
  flush,
  PANEL_WINDOW_ID,
  stateFor,
} from './test-support';

await bootPanel();

type Spy = ReturnType<typeof vi.fn>;
const firstResult = <T>(mount: unknown): T => {
  const value = vi.mocked(mount as () => unknown).mock.results[0]?.value as T | undefined;
  if (!value) throw new Error('a mount() was not called at module load');
  return value;
};

describe('sidepanel window scoping', () => {
  it('follows the active tab of its OWN window on load', async () => {
    await flush();
    expect(subscribeAnswerState).toHaveBeenCalledWith(7, expect.any(Function));
  });

  it('ignores a tabs.onActivated activation from a DIFFERENT window (regression)', async () => {
    await flush();
    vi.mocked(subscribeAnswerState).mockClear();

    activate(55, 999);
    await flush();

    expect(subscribeAnswerState).not.toHaveBeenCalled();
  });

  it('follows an activation in its OWN window', async () => {
    await flush();
    vi.mocked(subscribeAnswerState).mockClear();

    activate(42, PANEL_WINDOW_ID);
    await flush();

    expect(subscribeAnswerState).toHaveBeenCalledWith(42, expect.any(Function));
  });
});

// ── job-tools wiring (panel parity) ─────────────────────────────────────────
// `follow()` is BOTH of job-tools's own documented panel trigger points ("on
// mount and on tab activation" — see job-tools.ts's `JobToolsView.checkPage`
// doc): the initial call from `resolvePanelWindowId().then(...)` is the
// mount, every later call from `tabs.onActivated`/a focus change is an
// activation.

describe('job-tools wiring (panel parity)', () => {
  const jobTools = firstResult<{ render: Spy; checkPage: Spy }>(mountJobTools);

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

    activate(42);
    await flush();

    expect(jobTools.checkPage.mock.calls.length).toBeGreaterThan(callsAfterMount);
  });

  it('feeds the subscribed AnswerState to jobTools.render alongside answerTools.render', async () => {
    vi.mocked(subscribeAnswerState).mockClear();
    jobTools.render.mockClear();

    activate(42);
    await flush();

    const stateCallback = vi.mocked(subscribeAnswerState).mock.calls[0]?.[1];
    if (!stateCallback) throw new Error('subscribeAnswerState callback not captured');
    const fakeState = stateFor(42);
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
  const jobTools = firstResult<{ render: Spy; checkPage: Spy }>(mountJobTools);

  it("does not call checkPage before the tab's own state has actually been delivered", () => {
    const deliver = captureNextSubscription();
    jobTools.checkPage.mockClear();
    jobTools.render.mockClear();

    activate(99);

    // Nothing has been delivered yet — checkPage must not have fired against
    // whatever trust the PREVIOUS tab (or the cold-mount default) left behind.
    expect(jobTools.checkPage).not.toHaveBeenCalled();
    expect(jobTools.render).not.toHaveBeenCalled();

    // The async delivery lands (`deliver` fails loudly if the subscription was
    // never captured, rather than silently no-op'ing into a confusing
    // "expected N calls, got 0" a few lines down).
    deliver(stateFor(99));

    // render() must run BEFORE checkPage() reads the trust it just set.
    expect(jobTools.render).toHaveBeenCalledTimes(1);
    expect(jobTools.checkPage).toHaveBeenCalledTimes(1);
    const renderOrder = jobTools.render.mock.invocationCallOrder[0];
    const checkPageOrder = jobTools.checkPage.mock.invocationCallOrder[0];
    expect(renderOrder).toBeLessThan(checkPageOrder as number);
  });

  it('fires checkPage only on the FIRST delivery for a followed tab, not on later pushes for the same tab', () => {
    const deliver = captureNextSubscription();
    jobTools.checkPage.mockClear();

    activate(101);

    const state = stateFor(101);
    deliver(state); // first delivery — checkPage fires
    deliver({ ...state, scannedAt: 2 }); // a later push (e.g. an answer accepted)

    expect(jobTools.checkPage).toHaveBeenCalledTimes(1);
  });

  it('ignores a stale follow(A) callback that resolves after follow(B) has already superseded it', () => {
    const deliverA = captureNextSubscription();
    const deliverB = captureNextSubscription();
    jobTools.checkPage.mockClear();
    jobTools.render.mockClear();

    // follow(A) — its read is kicked off but not yet resolved.
    activate(201);
    // follow(B) supersedes A before A's read resolves. `subscribeAnswerState`'s
    // returned unsubscribe (called here) does NOT cancel A's in-flight read —
    // see `followGeneration`'s doc in sidepanel.ts for why that matters.
    activate(202);

    deliverB(stateFor(202));
    jobTools.render.mockClear();
    jobTools.checkPage.mockClear();

    // A's stale read finally resolves — it must be a complete no-op.
    deliverA(stateFor(201));

    expect(jobTools.render).not.toHaveBeenCalled();
    expect(jobTools.checkPage).not.toHaveBeenCalled();
  });
});

describe('the Answers tab count badge', () => {
  it('reflects the delivered state row count', async () => {
    deliverOnNextSubscription(
      stateFor(44, {
        rows: [
          { id: 'a', question: 'Q', field: null, status: 'empty', versions: [], selected: -1 },
        ],
      })
    );
    activate(44);
    await flush();

    expect(document.querySelector('[data-tab="answers"]')!.textContent).toBe('Answers (1)');
  });
});

// ── job-status refresh keyed by (origin, pageChanged) transitions (item 13) ─

describe('job-status refresh is keyed by transition, not every state push (item 13)', () => {
  it('refreshes once for N streamed updates that only change unrelated fields', async () => {
    const jobStatus = firstResult<{ refresh: Spy }>(mountJobStatus);
    jobStatus.refresh.mockClear();

    const deliver = captureNextSubscription();
    activate(601);

    const base = stateFor(601);
    deliver(base); // first delivery for this tab
    deliver({ ...base, scannedAt: 2 }); // streamed chunk 1
    deliver({ ...base, scannedAt: 3 }); // streamed chunk 2
    deliver({ ...base, scannedAt: 4 }); // streamed chunk 3

    expect(jobStatus.refresh).toHaveBeenCalledTimes(1);
  });
});

// ── #1225/#1234: documents + prep get the caller's shared gated line on an
// untrusted/no-tab state, and refresh only on a trusted delivery ─────────────

describe('documents/prep follow() trust gating (#1225, #1234)', () => {
  const documents = firstResult<{ refresh: Spy; reset: Spy }>(mountDocuments);
  const prep = firstResult<{ refresh: Spy; reset: Spy }>(mountPrep);
  const clearAll = () =>
    [documents.refresh, documents.reset, prep.refresh, prep.reset].forEach((s) => s.mockClear());

  it('refreshes documents + prep only on a trusted delivery; an untrusted (pageChanged) push resets both with the shared line', async () => {
    const deliver = captureNextSubscription();
    clearAll();

    activate(701);
    await flush();

    const base = stateFor(701);
    deliver(base);
    await flush();

    expect(documents.refresh).toHaveBeenCalledTimes(1);
    expect(prep.refresh).toHaveBeenCalledTimes(1);
    expect(documents.reset).not.toHaveBeenCalled();
    expect(prep.reset).not.toHaveBeenCalled();

    documents.refresh.mockClear();
    prep.refresh.mockClear();

    deliver({ ...base, pageChanged: true });
    await flush();

    expect(documents.refresh).not.toHaveBeenCalled();
    expect(prep.refresh).not.toHaveBeenCalled();
    expect(documents.reset).toHaveBeenCalledTimes(1);
    expect(prep.reset).toHaveBeenCalledTimes(1);
    expect(documents.reset).toHaveBeenCalledWith(JOB_TOOLS_GATED_LINE);
    expect(prep.reset).toHaveBeenCalledWith(JOB_TOOLS_GATED_LINE);
  });

  it('a no-tab follow (unresolved focus-change query) resets documents + prep with the shared line and never refreshes', async () => {
    clearAll();

    // `activeTabId()` reads the window's active tab — an empty query (no
    // active tab) drives follow(null), whose reset branch is the tabId-null
    // path (#1225's other reset site).
    vi.mocked(browser.tabs.query).mockResolvedValueOnce([]);
    const onFocusChanged = vi.mocked(browser.windows.onFocusChanged.addListener).mock.calls[0]?.[0];
    if (!onFocusChanged) throw new Error('windows.onFocusChanged listener not registered');
    onFocusChanged();
    await flush();

    expect(documents.reset).toHaveBeenCalledTimes(1);
    expect(prep.reset).toHaveBeenCalledTimes(1);
    expect(documents.reset).toHaveBeenCalledWith(JOB_TOOLS_GATED_LINE);
    expect(prep.reset).toHaveBeenCalledWith(JOB_TOOLS_GATED_LINE);
    expect(documents.refresh).not.toHaveBeenCalled();
    expect(prep.refresh).not.toHaveBeenCalled();
  });
});

describe('a followed-job change clears the "already applied" tick', () => {
  const tools = firstResult<{ clearApplied: Spy }>(mountJobTools);

  it('on every follow(), and on an origin change within the same tab, but not on a same-job push', () => {
    const deliver = captureNextSubscription();
    tools.clearApplied.mockClear();

    activate(120);
    expect(tools.clearApplied).toHaveBeenCalledTimes(1); // tab switch

    const state = stateFor(120);
    deliver(state); // first delivery: key changes from null
    expect(tools.clearApplied).toHaveBeenCalledTimes(2);
    deliver({ ...state, scannedAt: 2 }); // same job, later push
    expect(tools.clearApplied).toHaveBeenCalledTimes(2);
    deliver({ ...state, origin: 'https://other.example.com' }); // navigated elsewhere
    expect(tools.clearApplied).toHaveBeenCalledTimes(3);
  });
});
