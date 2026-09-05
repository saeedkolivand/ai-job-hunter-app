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

vi.mock('../lib/answer-state', () => ({
  subscribeAnswerState: vi.fn(() => vi.fn()),
}));

const PANEL_WINDOW_ID = 100;

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    runtime: { sendMessage: vi.fn() },
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

document.body.innerHTML = '<div id="job-tools-host"></div><div id="answer-tools-host"></div>';

const { subscribeAnswerState } = await import('../lib/answer-state');
const { mountJobTools } = await import('../job-tools/job-tools');
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
