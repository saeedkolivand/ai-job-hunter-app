import { describe, expect, it, vi } from 'vitest';

import type { BridgeClient } from '../bridge';
import {
  connectedClient,
  frameAt,
  outcomeOf,
  setupFakeWebSocket,
  T,
  unauthenticatedClient,
} from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();
const URL_X = 'https://jobs.example.com/posting/x';
const NOT_REACHABLE = 'Desktop app not reachable. Is AI Job Hunter running?';

/**
 * No stored token: the attach reaches `phase === 'connected'` with NO handshake.
 * Every verb must treat that as "not reachable" and put nothing on the wire.
 */
describe('BridgeClient – verbs before mutual auth (no stored token)', () => {
  it.each<[string, (client: BridgeClient) => Promise<unknown>]>([
    ['importJob', (c) => c.importJob({ url: URL_X, applied: false })],
    ['getProfile', (c) => c.getProfile()],
    ['checkApplied', (c) => c.checkApplied(URL_X)],
    ['updateStatus', (c) => c.updateStatus(URL_X)],
    ['saveAnswers', (c) => c.saveAnswers(URL_X, [])],
    ['suggestAnswers', (c) => c.suggestAnswers(['Why this role?'])],
    ['settingsGet', (c) => c.settingsGet()],
    ['agentQuery', (c) => c.agentQuery('job')],
    ['answerAssist', (c) => c.answerAssist({ question: 'Why this role?' })],
  ])('%s rejects with the existing not-reachable error and sends no frame', async (_v, call) => {
    const { client, socket } = await unauthenticatedClient(fake);
    expect(client.status()).toMatchObject({ phase: 'connected', authenticated: false });

    const outcome = await outcomeOf(call(client));

    expect(outcome.ok).toBe(false);
    if (!outcome.ok) expect((outcome.error as Error).message).toBe(NOT_REACHABLE);
    expect(socket.send).not.toHaveBeenCalled();
    client.dispose();
  });

  it.each<[string, (client: BridgeClient) => Promise<boolean>]>([
    ['autotrackEnabled', (c) => c.autotrackEnabled()],
    ['autofillEnabled', (c) => c.autofillEnabled()],
  ])('%s still never rejects: degrades to false without a frame', async (_v, call) => {
    const { client, socket } = await unauthenticatedClient(fake);
    expect(await call(client)).toBe(false);
    expect(socket.send).not.toHaveBeenCalled();
    client.dispose();
  });

  it('cancelAssist sends no assist.cancel before auth', async () => {
    const { client, socket } = await unauthenticatedClient(fake);
    client.cancelAssist('some-req-id');
    expect(socket.send).not.toHaveBeenCalled();
    client.dispose();
  });

  it('cancelAssist DOES send assist.cancel on an authenticated session (positive control)', async () => {
    const { client, socket } = await connectedClient(fake);
    client.cancelAssist('some-req-id');
    expect(frameAt(socket)).toMatchObject({ type: T.assistCancel, reqId: 'some-req-id' });
    client.dispose();
  });
});
