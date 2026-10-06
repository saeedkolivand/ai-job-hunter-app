/** `stampResults` — results-page batch stamping (PR3 §B.4). */

import { beforeEach, describe, expect, it } from 'vitest';

import {
  activeTab,
  DESKTOP_DOWN,
  executeScriptMock,
  FAKE_TOKEN,
  getTokenMock,
  mockClient,
  resetMocks,
  scriptResults,
  send,
  setStampResultsPages,
} from './test-support';

beforeEach(resetMocks);

const CARD = { url: 'https://x/jobs/1', index: 0 };

describe('stampResults request — preference short-circuit', () => {
  it('refuses when the results-stamp preference is off (defense in depth against a stale UI)', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    await setStampResultsPages(false);

    const res = await send({ kind: 'stampResults' });

    expect(res.ok).toBe(false);
    expect(executeScriptMock).not.toHaveBeenCalled();
  });
});

describe('stampResults request', () => {
  beforeEach(async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    await setStampResultsPages(true);
    activeTab('https://x/jobs?q=data');
  });

  const stamped = (stampedCount: number, status: string) => ({
    ok: true,
    kind: 'stampResults',
    stamped: stampedCount,
    status,
  });

  it('collects candidate cards, batch-checks them, stamps, and reports the count', async () => {
    scriptResults(undefined, [CARD], 1); // results-stamp.js files, collect func, stamp func
    mockClient.checkAppliedBatch.mockResolvedValue({
      ok: true,
      results: [{ url: CARD.url, found: true, status: 'saved' }],
    });

    const res = await send({ kind: 'stampResults' });

    expect(mockClient.checkAppliedBatch).toHaveBeenCalledWith([CARD.url]);
    expect(res).toEqual(stamped(1, 'Stamped 1 card.'));
  });

  it('degrades a desktop-side refusal (over-cap/throttle) to "no stamps", never a partial lie', async () => {
    scriptResults(undefined, [CARD]);
    mockClient.checkAppliedBatch.mockResolvedValue({ ok: false, error: 'too_many_urls' });

    const res = await send({ kind: 'stampResults' });

    expect(res).toEqual(stamped(0, 'too_many_urls'));
    // No stamp step is ever reached once the batch itself was refused.
    expect(executeScriptMock).toHaveBeenCalledTimes(2);
  });

  it('reports "no job cards" without ever calling the bridge when the page has none', async () => {
    scriptResults(undefined, []);

    const res = await send({ kind: 'stampResults' });

    expect(mockClient.checkAppliedBatch).not.toHaveBeenCalled();
    expect(res).toEqual(stamped(0, 'No job cards found on this page.'));
  });

  it('degrades a transport rejection on the batch call to "no stamps" too', async () => {
    scriptResults(undefined, [CARD]);
    mockClient.checkAppliedBatch.mockRejectedValue(new Error(DESKTOP_DOWN));

    const res = await send({ kind: 'stampResults' });

    expect(res).toEqual(stamped(0, 'Could not reach the desktop app.'));
  });
});
