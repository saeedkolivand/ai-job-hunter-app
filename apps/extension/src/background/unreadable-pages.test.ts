/**
 * Permanently-unreadable pages (#1219) — the SHARED refusal for "Check fit" and
 * "Stamp this results page": a truthful "can't be read" message, never the
 * transient reload hint, which is reserved for failures on readable pages.
 */

import { beforeEach, describe, expect, it } from 'vitest';

import {
  activeTab,
  executeScriptMock,
  FAKE_TOKEN,
  getTokenMock,
  mockClient,
  RELOAD_HINT,
  resetMocks,
  scriptResults,
  send,
  setStampResultsPages,
  tabsQueryMock,
  UNREADABLE_PAGE,
  UNREADABLE_URLS,
} from './test-support';

beforeEach(resetMocks);

describe.each([
  {
    kind: 'matchLive',
    bridgeCall: () => mockClient.matchLive,
    arrange: async () => {},
    // The page DOM capture returns nothing → throws.
    transientFailure: () => scriptResults(null),
  },
  {
    kind: 'stampResults',
    bridgeCall: () => mockClient.checkAppliedBatch,
    arrange: () => setStampResultsPages(true),
    // results-stamp.js files, then the collect func → null → throws.
    transientFailure: () => scriptResults(undefined, null),
  },
] as const)('$kind request — permanently-unreadable pages (#1219)', (gesture) => {
  beforeEach(async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    await gesture.arrange();
  });

  const expectUntouched = () => {
    expect(executeScriptMock).not.toHaveBeenCalled();
    expect(gesture.bridgeCall()).not.toHaveBeenCalled();
  };

  it.each(UNREADABLE_URLS)(
    'answers %s with the unreadable-page message, without reading the page or calling the bridge',
    async (_label, url) => {
      activeTab(url);

      expect(await send({ kind: gesture.kind })).toEqual(UNREADABLE_PAGE);
      expectUntouched();
    }
  );

  it('answers the redacted-empty-url case (Chrome hides restricted tab urls without `tabs` permission)', async () => {
    activeTab('');

    expect(await send({ kind: gesture.kind })).toEqual(UNREADABLE_PAGE);
    expectUntouched();
  });

  it('answers the no-active-tab case with the same message', async () => {
    tabsQueryMock.mockResolvedValue([]);

    expect(await send({ kind: gesture.kind })).toEqual(UNREADABLE_PAGE);
    expectUntouched();
  });

  it('keeps the transient reload hint for a capture failure on a NORMAL, readable page', async () => {
    // The reversed assertion: on a page that CAN be read, a failure still gets
    // the reload hint — never the unreadable-page message, which would be a lie.
    activeTab();
    gesture.transientFailure();

    expect(await send({ kind: gesture.kind })).toEqual(RELOAD_HINT);
    expect(gesture.bridgeCall()).not.toHaveBeenCalled();
  });
});
