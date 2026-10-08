/**
 * Unit tests for the side panel's own chrome and consent gates (sidepanel.ts):
 * the connection-status composition, gear button, tab bar, trust line, the
 * first-time Fill/Attach confirmation binding, and auto-track's pushed
 * `jobStatusChanged`. Collaborators are recording stubs (see `test-mocks.ts`).
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

import { mountConnectionStatus } from '../../connection-status/connection-status';
import { mountDocuments } from '../../documents/documents';
import { mountJobStatus } from '../../job-status/job-status';
import type * as JobToolsModule from '../../job-tools/job-tools';
import { mountJobTools } from '../../job-tools/job-tools';
import { mountFirstFillConfirm } from '../../lib/site-memory';
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

const byId = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const tabButton = (tab: string) =>
  document.querySelector<HTMLButtonElement>(`[data-tab="${tab}"]`)!;
const onStatus = () => {
  const dep = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onStatus;
  if (!dep) throw new Error('onStatus dep not captured');
  return dep;
};
const fillConfirm = () => {
  const value = vi.mocked(mountFirstFillConfirm).mock.results[0]?.value;
  if (!value) throw new Error('mountFirstFillConfirm was not called at module load');
  return value;
};

// ── connection-status composition (ADR-046) ─────────────────────────────────
// The panel's ONLY connection-status responsibility: show `#view-connected`
// (the job/answer tools) only while `phase === 'connected'`. The pill/retry/
// pairing/offline/outdated/searching behavior itself lives in
// `connection-status.ts` — see `connection-status/connection-status/status.test.ts` for that.

describe('connection-status composition', () => {
  it('mounts against the pill + views hosts and starts it', () => {
    expect(vi.mocked(mountConnectionStatus)).toHaveBeenCalledWith(
      byId('connection-pill-host'),
      byId('connection-views-host'),
      expect.objectContaining({ send: expect.any(Function), onStatus: expect.any(Function) })
    );
    const view = vi.mocked(mountConnectionStatus).mock.results[0]?.value as
      { start: ReturnType<typeof vi.fn> } | undefined;
    expect(view?.start).toHaveBeenCalledTimes(1);
  });

  it('shows #view-connected only while phase === connected', () => {
    const viewConnected = byId('view-connected');

    onStatus()({ phase: 'connected', port: 1, hasToken: true });
    expect(viewConnected.hidden).toBe(false);

    onStatus()({ phase: 'app_not_running', port: null, hasToken: true });
    expect(viewConnected.hidden).toBe(true);

    onStatus()({ phase: 'searching', port: null, hasToken: false });
    expect(viewConnected.hidden).toBe(true);
  });
});

// ── PR0 §3: the gear button, the tab bar ────────────────────────────────────

describe('the gear button opens the Settings page', () => {
  it('calls browser.runtime.openOptionsPage on click', () => {
    byId('btn-settings').dispatchEvent(new Event('click', { bubbles: true }));
    expect(browser.runtime.openOptionsPage).toHaveBeenCalled();
  });
});

describe('the tab bar (Job / Documents / Answers / Prep)', () => {
  it('mounts four tabs, Job active by default', () => {
    expect(document.querySelectorAll<HTMLButtonElement>('.tab')).toHaveLength(4);
    expect(tabButton('job').classList).toContain('active');
  });

  it('mounts job-status + job-tools into the Job panel, the Documents host into the Documents panel, answer-tools into the Answers panel, and the Prep host into the Prep panel', () => {
    const panel = (section: string) =>
      document.querySelector<HTMLElement>(`[data-section="${section}"]`)!;
    expect(panel('job').querySelector('#job-tools-host')).not.toBeNull();
    expect(panel('documents').querySelector('#documents-host')).not.toBeNull();
    expect(panel('answers').querySelector('#answer-tools-host')).not.toBeNull();
    expect(panel('prep').querySelector('#prep-host')).not.toBeNull();
  });
});

describe('the trust line (ADR-045)', () => {
  const trustLine = () => byId('trust-line');
  const sendMessage = vi.mocked(browser.runtime.sendMessage);

  /** Follow `tabId` whose state arrives for `origin`, then let the read tier settle. */
  async function followTrusted(tabId: number, over: Record<string, unknown> = {}, ticks = 1) {
    deliverOnNextSubscription(stateFor(tabId, over));
    activate(tabId);
    for (let i = 0; i < ticks; i += 1) await flush();
  }

  it('shows "Reading: <host>" once a trusted state is delivered', async () => {
    await followTrusted(42);

    expect(trustLine().hidden).toBe(false);
    expect(trustLine().textContent).toBe('Reading: jobs.example.com');
  });

  it('upgrades the host-only line to "Reading: <title> · <company>" once the read tier answers (PR1)', async () => {
    sendMessage.mockClear();
    sendMessage.mockResolvedValueOnce({
      ok: true,
      kind: 'trustLineJob',
      title: 'Senior Rust Engineer',
      company: 'Acme',
    });

    // The synchronous host-only line renders first (never blocks), then the
    // async agentQuery answer upgrades it.
    await followTrusted(45, {}, 2);

    // Carries the panel's own window id so the background reads THIS window's
    // active tab rather than the last-focused window's (#1215).
    expect(browser.runtime.sendMessage).toHaveBeenCalledWith({
      kind: 'trustLineJob',
      windowId: PANEL_WINDOW_ID,
    });
    expect(trustLine().textContent).toBe('Reading: Senior Rust Engineer · Acme');
  });

  it('keeps the host-only line on a read-tier refusal (Autofill off, throttled, unknown job)', async () => {
    sendMessage.mockClear();
    sendMessage.mockResolvedValueOnce({
      ok: true,
      kind: 'trustLineJob',
      title: null,
      company: null,
    });

    await followTrusted(46, {}, 2);

    expect(trustLine().hidden).toBe(false);
    expect(trustLine().textContent).toBe('Reading: jobs.example.com');
  });

  it('never lands a stale trustLineJob reply for a tab the panel has since left (follow(A) → follow(B))', async () => {
    // Tab A's trustLineJob query is kicked off but deliberately never
    // resolves until later in this test — captured so it can be settled
    // AFTER follow(B) has already superseded it.
    let resolveA: ((value: unknown) => void) | undefined;
    sendMessage.mockClear();
    sendMessage.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveA = resolve;
        })
    );

    // follow(A) — the host-only line renders synchronously; its trustLineJob
    // query is in flight but held open by `resolveA` above.
    await followTrusted(901);
    expect(trustLine().textContent).toBe('Reading: jobs.example.com');

    // follow(B) — a fresh tab activation supersedes A before A's query
    // resolves (bumps `trustLineJobGeneration`, invalidating A's in-flight
    // read — sidepanel.ts's own `follow()` doc).
    await followTrusted(902, { origin: 'https://other.example.com' });
    expect(trustLine().textContent).toBe('Reading: other.example.com');

    // Tab A's stale trustLineJob reply finally resolves — must be a no-op.
    resolveA?.({ ok: true, kind: 'trustLineJob', title: 'Stale Job', company: 'Stale Co' });
    await flush();

    expect(trustLine().textContent).toBe('Reading: other.example.com');
    expect(trustLine().textContent).not.toContain('Stale Job');
  });

  it('hides again for an untrusted (pageChanged) state, leaving the message to job-tools', async () => {
    await followTrusted(43, { pageChanged: true });

    expect(trustLine().hidden).toBe(true);
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
    vi.mocked(fillConfirm().cancel).mockClear();

    activate(501);

    expect(fillConfirm().cancel).toHaveBeenCalledTimes(1);
  });

  it('re-validates the captured origin/generation after confirm() resolves, aborting a stale confirmation even when the user answered Fill', async () => {
    await flush();
    const jobToolsCall = vi.mocked(mountJobTools).mock.calls[0] as unknown as [
      HTMLElement,
      JobToolsModule.JobToolsDeps,
    ];
    const confirmFill = jobToolsCall[1].confirmFill;
    if (!confirmFill) throw new Error('confirmFill dep not passed to mountJobTools');

    let resolveConfirm: ((v: boolean) => void) | undefined;
    vi.mocked(fillConfirm().confirm).mockReturnValueOnce(
      new Promise<boolean>((resolve) => {
        resolveConfirm = resolve;
      })
    );

    const pending = confirmFill();

    // The panel follows a DIFFERENT tab while the confirmation is still open
    // (bumps the follow generation and, in real usage, calls cancel() too —
    // this test's own assertion is about sidepanel.ts's re-validation, which
    // must hold regardless of whether the mocked confirm() ever "hears" it).
    activate(502);
    await flush();

    // The user eventually answers "Fill" on the now-stale confirmation.
    resolveConfirm?.(true);

    await expect(pending).resolves.toBe(false);
  });
});

// ── #1224: the first-time Fill confirmation host lives OUTSIDE the tab
// panels ─────────────────────────────────────────────────────────────────────
// Inside a panel the inset was hidden by `tabs.ts`'s `setActive` whenever
// that tab wasn't active (Chrome sets `[hidden]` on the other panels), so
// from the Documents tab the Attach confirmation was invisible and its
// promise never resolved. The fix moves the host to a SIBLING of the tab
// bar inside #view-connected — outside every `[data-section]` panel.

describe('the Fill confirmation host is mounted outside the tab panels (#1224)', () => {
  it('hangs off #view-connected (never a [data-section] panel) and stays visible from the Documents tab', async () => {
    await flush();
    const host = vi.mocked(mountFirstFillConfirm).mock.calls[0]?.[0];
    if (!host) throw new Error('mountFirstFillConfirm host not captured');

    expect(host.parentElement?.id).toBe('view-connected');
    expect(host.closest('[data-section]')).toBeNull();

    // Connected + Documents tab active: the host (and the inset it owns)
    // must not be hidden by the tab machinery.
    onStatus()({ phase: 'connected', port: 1, hasToken: true });
    tabButton('documents').click();

    expect(host.closest('[hidden]')).toBeNull();
  });
});

// ── auto-track's pushed jobStatusChanged (#1233) ────────────────────────────
// The background broadcasts a CONFIRMED saved→applied flip over the shared
// `runtime.sendMessage` channel (the same one `broadcastStatus` uses); the
// panel's ONE module-scope listener must refresh ONLY the followed page's own
// job, never another window's, and never on a malformed url. Registered at
// module load — `mock.calls[0]` on `runtime.onMessage.addListener` is this
// listener (connection-status's own listener lives in `connection-status.ts`,
// which this file mocks out, so nothing else registers one here).

describe('auto-track pushed jobStatusChanged (#1233)', () => {
  const refresh = () => vi.mocked(mountJobStatus).mock.results[0]?.value.refresh;

  /** Follow a trusted tab and let `currentOrigin` settle to `origin` BEFORE
   *  the push under test fires. follow()'s first-delivery key-change path
   *  itself calls `jobStatus.refresh()`, so the mock is cleared right after
   *  delivery — the assertions below then count ONLY the push-driven calls. */
  async function followWithOrigin(origin: string, tabId = 701): Promise<void> {
    deliverOnNextSubscription(stateFor(tabId, { origin }));
    activate(tabId);
    await flush();
    vi.mocked(refresh()).mockClear();
  }

  function pushedListener(): (message: unknown) => void {
    const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0];
    if (!listener) throw new Error('jobStatusChanged listener not registered');
    return listener;
  }

  it('refreshes the job-status card for a flip on the FOLLOWED page origin', async () => {
    await followWithOrigin('https://jobs.example.com');
    pushedListener()({
      ok: true,
      kind: 'jobStatusChanged',
      url: 'https://jobs.example.com/posting/1',
    } as never);
    expect(refresh()).toHaveBeenCalledTimes(1);
  });

  it('ignores a flip for a DIFFERENT origin — another window/tab, never on screen', async () => {
    await followWithOrigin('https://jobs.example.com');
    pushedListener()({
      ok: true,
      kind: 'jobStatusChanged',
      url: 'https://other.example.com/posting/2',
    } as never);
    expect(refresh()).not.toHaveBeenCalled();
  });

  it('ignores a malformed url — never refreshes on garbage from our own background', async () => {
    await followWithOrigin('https://jobs.example.com');
    pushedListener()({ ok: true, kind: 'jobStatusChanged', url: 'not a url' } as never);
    expect(refresh()).not.toHaveBeenCalled();
  });

  it('ignores non-jobStatusChanged pushes (status, ok:false replies) on the shared channel', async () => {
    await followWithOrigin('https://jobs.example.com');
    const listener = pushedListener();
    listener({
      ok: true,
      kind: 'status',
      status: { phase: 'connected', port: 1, hasToken: true },
    } as never);
    listener({ ok: false, error: 'no such request' } as never);
    expect(refresh()).not.toHaveBeenCalled();
  });
});

// ── #1249: no confirmation bypass before the first state delivery ──────────
// The panel's `currentOrigin` is fed only by `follow()`'s answer-state
// subscription, so it is `null` until the first push lands. `confirm(null)`
// used to resolve TRUE, so a Fill or Attach clicked in that window wrote into
// a page the panel could not name, with no dialog shown. The popup's fix
// (resolve the origin on demand) does not transfer here: the panel follows
// arbitrary tabs and `activeTab` is gesture-scoped, so there is no grant to
// read a followed tab's url from.

describe('first-time confirmation with no origin yet (#1249)', () => {
  const deps = vi.mocked(mountJobTools).mock.calls[0]?.[1] as
    { confirmFill?: () => Promise<boolean> } | undefined;
  if (!deps) throw new Error('mountJobTools was not called at module load');

  const docDeps = vi.mocked(mountDocuments).mock.calls[0]?.[1] as
    | {
        confirmAttach?: (host: string | null) => Promise<boolean>;
        currentHost?: () => string | null;
      }
    | undefined;
  if (!docDeps) throw new Error('mountDocuments was not called at module load');

  /**
   * Drive the panel into the real pre-first-delivery state: follow a fresh
   * tab, then deliver `null` (no answer state for it yet) — which is exactly
   * what `follow()`'s subscription hands the panel before a scan lands.
   * Module state persists across tests in this file, so the precondition is
   * established explicitly rather than assumed.
   */
  function followTabWithNoState(tabId: number): void {
    const deliver = captureNextSubscription();
    activate(tabId);
    deliver(null);
  }

  it('refuses Fill rather than approving a page it cannot name', async () => {
    followTabWithNoState(901);
    await expect(deps.confirmFill?.()).resolves.toBe(false);
  });

  it('refuses Attach for the same unknown host', async () => {
    followTabWithNoState(902);
    expect(docDeps.currentHost?.()).toBeNull();
    await expect(docDeps.confirmAttach?.(docDeps.currentHost?.() ?? null)).resolves.toBe(false);
  });

  it('still asks normally once an origin IS known', async () => {
    // The guard must not have turned into "never confirm" — with a real
    // origin the inset is shown and the promise stays pending until answered.
    const deliver = captureNextSubscription();
    activate(903);
    deliver(stateFor(903));
    expect(docDeps.currentHost?.()).toBe('jobs.example.com');
  });
});

describe('applied-check parity with the popup', () => {
  it('feeds every job-status outcome into the job tools (Import label + applied box)', () => {
    const onResult = vi.mocked(mountJobStatus).mock.calls[0]?.[1].onResult;
    const tools = vi.mocked(mountJobTools).mock.results[0]?.value;
    const res = {
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'applied' },
    } as never;
    onResult?.(res);
    expect(tools.applyAppliedCheck).toHaveBeenCalledWith(res);
  });
});
