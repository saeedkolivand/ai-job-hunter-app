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

document.body.innerHTML = '<div id="answer-tools-host"></div>';

const { subscribeAnswerState } = await import('../lib/answer-state');
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
