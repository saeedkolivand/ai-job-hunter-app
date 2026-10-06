/** `getStatus` — the popup-facing connection status and the import-prompt badge. */

import { beforeEach, describe, expect, it } from 'vitest';

import {
  FAKE_TOKEN,
  getTokenMock,
  mockClient,
  resetMocks,
  send,
  setBadgeTextMock,
} from './test-support';

beforeEach(resetMocks);

describe('getStatus clears the import/badge prompt (Task #22 review closure)', () => {
  it('clears the action badge set by a prior untracked-submit nudge', async () => {
    await send({ kind: 'getStatus' });

    expect(setBadgeTextMock).toHaveBeenCalledWith({ text: '' });
  });
});

// #1267 — computeStatus() must reflect AUTHENTICATION, not the raw bridge phase:
// `bridge.phase === 'connected'` is also reached with zero handshake (the
// no-token attach path, and briefly right after a fresh token is saved, before
// the forced re-handshake settles).
describe('computeStatus folds bridge.authenticated into the popup phase (#1267)', () => {
  it.each([
    [
      'reports "searching" (never "connected") while a token is stored but the transport has not authenticated',
      false,
      'searching',
    ],
    ['reports "connected" once the same phase is actually authenticated', true, 'connected'],
  ])('%s', async (_label, authenticated, phase) => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.status.mockReturnValueOnce({ phase: 'connected', port: 47615, authenticated });

    const res = await send({ kind: 'getStatus' });

    if (!res.ok || res.kind !== 'status') throw new Error('expected a status response');
    expect(res.status.phase).toBe(phase);
  });
});
