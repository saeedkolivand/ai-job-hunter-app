/** The worker's `onMessage` listener only serves PopupRequests to extension PAGES, never to content scripts. */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import { EXTENSION_ID, flush, listener, mockClient, resetMocks } from './test-support';

beforeEach(resetMocks);

const ORIGIN = `chrome-extension://${EXTENSION_ID}`;

/** Hand the raw listener a PopupRequest from `sender`; report what it did. */
async function dispatch(sender: Record<string, unknown>) {
  mockClient.ensureConnected.mockClear();
  const sendResponse = vi.fn();
  const kept = listener?.({ kind: 'getStatus' }, sender as never, sendResponse);
  await flush();
  return { kept, sendResponse };
}

describe('PopupRequest sender gate', () => {
  it.each<[string, Record<string, unknown>]>([
    ['the popup', { id: EXTENSION_ID, url: `${ORIGIN}/popup.html` }],
    ['the side panel', { id: EXTENSION_ID, url: `${ORIGIN}/sidepanel.html` }],
    // An options page opened in a tab HAS `sender.tab` — it must still be served.
    [
      'an options page opened in a tab',
      { id: EXTENSION_ID, url: `${ORIGIN}/options.html`, tab: { id: 3 } },
    ],
  ])('serves a request from %s', async (_name, sender) => {
    const { kept, sendResponse } = await dispatch(sender);
    expect(kept).toBe(true);
    expect(sendResponse).toHaveBeenCalledWith(
      expect.objectContaining({ ok: true, kind: 'status' })
    );
  });

  it.each<[string, Record<string, unknown>]>([
    [
      'a content script (page url + tab)',
      { id: EXTENSION_ID, url: 'https://jobs.example.com/posting/9', tab: { id: 1 } },
    ],
    ['our own url but a foreign sender id', { id: 'other-extension', url: `${ORIGIN}/popup.html` }],
    ['a sender with no url', { id: EXTENSION_ID }],
    [
      'a foreign extension page',
      { id: 'other-extension', url: 'chrome-extension://other-extension/popup.html' },
    ],
  ])('ignores a request from %s: no response, no side effect', async (_name, sender) => {
    const { kept, sendResponse } = await dispatch(sender);
    expect(kept).toBeUndefined();
    expect(sendResponse).not.toHaveBeenCalled();
    expect(mockClient.ensureConnected).not.toHaveBeenCalled();
  });
});
