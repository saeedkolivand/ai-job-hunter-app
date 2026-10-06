/**
 * #1215 — a surface's gesture acts on ITS OWN window's tab, not the
 * last-focused window's (a service worker has no window of its own).
 */

import { beforeEach, describe, expect, it } from 'vitest';

import {
  EMAIL_SUMMARY,
  executeScriptMock,
  FAKE_TOKEN,
  flush,
  getTokenMock,
  matchOk,
  mockClient,
  POSTING_URL,
  resetMocks,
  scriptResults,
  send,
  tabsQueryMock,
} from './test-support';

beforeEach(async () => {
  await resetMocks();
  getTokenMock.mockResolvedValue(FAKE_TOKEN);
  fakeTwoWindowQuery();
});

/** The popup/panel's own window (window A) — the one the browser did NOT focus
 *  last, so a `currentWindow: true` query would have resolved to window B. Its
 *  active tab is a real job posting; `PopupRequest.windowId` must route every
 *  gesture here. */
const REQUESTING_WINDOW_ID = 1001;
/** The last-focused window (window B) — a DIFFERENT page entirely. The pre-#1215
 *  behavior (`currentWindow: true`) resolved to this one, which is how Import
 *  silently created an application from an unrelated page while reporting
 *  success. */
const FOCUSED_WINDOW_ID = 2002;
const REQUESTING_WINDOW_TAB = { id: 7, windowId: REQUESTING_WINDOW_ID, url: POSTING_URL } as never;
const FOCUSED_WINDOW_TAB = {
  id: 8,
  windowId: FOCUSED_WINDOW_ID,
  url: 'https://unrelated.example.com/page',
} as never;

/**
 * Fake `tabs.query` that distinguishes the two query shapes a real browser
 * answers differently: an explicit `windowId` resolves to THAT window's active
 * tab, `currentWindow: true` resolves to whichever window was focused LAST.
 * Without this distinction the bug could never be exercised — both shapes would
 * return the same tab and every test here would pass either way.
 */
function fakeTwoWindowQuery(): void {
  tabsQueryMock.mockImplementation(async (queryInfo) => {
    const q = queryInfo as { windowId?: number; currentWindow?: boolean };
    if (typeof q.windowId === 'number') {
      return [q.windowId === REQUESTING_WINDOW_ID ? REQUESTING_WINDOW_TAB : FOCUSED_WINDOW_TAB];
    }
    return [FOCUSED_WINDOW_TAB]; // `currentWindow: true` → the last-focused window
  });
}

const askedForRequestingWindow = () =>
  expect(tabsQueryMock).toHaveBeenCalledWith({ active: true, windowId: REQUESTING_WINDOW_ID });

describe('#1215 — a request carrying windowId acts on the requesting window tab, not the last-focused window tab', () => {
  it('import sends the requesting window url on the wire (the data-corruption path)', async () => {
    scriptResults('<html>job</html>');
    mockClient.importJob.mockResolvedValue({ ok: true, applicationId: 'app-1' });

    const res = await send({ kind: 'import', applied: false, windowId: REQUESTING_WINDOW_ID });

    expect(res.ok).toBe(true);
    // The seam must ask for the REQUESTING window explicitly — never
    // `currentWindow: true`, which would resolve the focused window's tab.
    askedForRequestingWindow();
    // The wire payload is the whole point of this fix: the OLD code resolved the
    // focused window's url and silently created an application from it.
    expect(mockClient.importJob).toHaveBeenCalledWith({
      url: POSTING_URL,
      applied: false,
      html: '<html>job</html>',
    });
  });

  it('fill injects into the requesting window tab, not the focused window tab', async () => {
    mockClient.getProfile.mockResolvedValue({ email: 'saeed@example.com' });
    scriptResults(undefined, EMAIL_SUMMARY);

    const res = await send({ kind: 'fill', windowId: REQUESTING_WINDOW_ID });

    expect(res).toEqual({ ok: true, kind: 'fill', summary: EMAIL_SUMMARY });
    expect(executeScriptMock).toHaveBeenCalledWith({ target: { tabId: 7 }, files: ['fill.js'] });
    // Never the focused window's tab (id 8) — that is the pre-fix mistarget.
    expect(executeScriptMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 8 } })
    );
  });

  it('matchLive captures and scores the requesting window tab', async () => {
    scriptResults('<html>job</html>');
    mockClient.matchLive.mockResolvedValue(matchOk({ combined: 72 }));

    const res = await send({ kind: 'matchLive', windowId: REQUESTING_WINDOW_ID });

    expect(res.ok).toBe(true);
    askedForRequestingWindow();
    expect(executeScriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 7 }, files: ['content.js'] })
    );
    expect(mockClient.matchLive).toHaveBeenCalledWith({
      url: POSTING_URL,
      html: '<html>job</html>',
    });
  });

  it('a request WITHOUT windowId still resolves via currentWindow: true, unchanged', async () => {
    mockClient.getProfile.mockResolvedValue({ email: 'saeed@example.com' });
    scriptResults(undefined, EMAIL_SUMMARY);

    // No `windowId` — an older surface build, or a flow with no originating
    // window at all (a context-menu click carries its own tab instead).
    const res = await send({ kind: 'fill' });

    expect(res).toEqual({ ok: true, kind: 'fill', summary: EMAIL_SUMMARY });
    expect(tabsQueryMock).toHaveBeenCalledWith({ active: true, currentWindow: true });
    expect(executeScriptMock).toHaveBeenCalledWith({ target: { tabId: 8 }, files: ['fill.js'] });
  });

  it('the submit watcher is armed against the requesting window tab', async () => {
    mockClient.getProfile.mockResolvedValue({ email: 'saeed@example.com' });
    mockClient.autotrackEnabled.mockResolvedValue(true);
    scriptResults(undefined, EMAIL_SUMMARY);

    await send({ kind: 'fill', windowId: REQUESTING_WINDOW_ID });
    await flush(); // the arm is fire-and-forget — flush it before asserting

    // Armed on the requesting window's tab (7), never the focused window's (8).
    expect(executeScriptMock).toHaveBeenCalledWith({
      target: { tabId: 7 },
      files: ['submit-watch.js'],
    });
    expect(executeScriptMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 8 }, files: ['submit-watch.js'] })
    );
  });
});
