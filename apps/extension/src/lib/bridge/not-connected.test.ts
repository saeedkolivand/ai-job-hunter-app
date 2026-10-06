import { describe, expect, it, vi } from 'vitest';

import { BridgeClient } from '../bridge';
import { failAllPorts, outcomeOf, setupFakeWebSocket } from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();

const URL_X = 'https://jobs.example.com/posting/x';

describe('BridgeClient – verbs while the desktop is unreachable', () => {
  it.each<[string, (client: BridgeClient) => Promise<unknown>]>([
    ['updateStatus', (c) => c.updateStatus(URL_X)],
    ['saveAnswers', (c) => c.saveAnswers(URL_X, [])],
    ['suggestAnswers', (c) => c.suggestAnswers(['Why this role?'])],
    ['matchLive', (c) => c.matchLive({ url: URL_X, html: '<html>job</html>' })],
    ['answerAssist', (c) => c.answerAssist({ question: 'Why this role?' })],
    ['agentQuery', (c) => c.agentQuery('job')],
    [
      'documentExport',
      (c) =>
        c.documentExport({
          source: { kind: 'generation', url: URL_X },
          kind: 'resume',
          format: 'pdf',
          templateId: 'classic',
        }),
    ],
  ])(
    '%s rejects when not connected — every port fails and the ws probe exhausts',
    async (_verb, call) => {
      vi.useFakeTimers();
      const client = new BridgeClient(vi.fn());
      // Attach synchronously so vitest never flags a transient "unhandled
      // rejection" while the probe below runs to completion.
      const outcomePromise = outcomeOf(call(client));
      await failAllPorts(fake);

      const outcome = await outcomePromise;
      expect(outcome.ok).toBe(false);
      if (!outcome.ok) {
        expect(outcome.error).toBeInstanceOf(Error);
        expect((outcome.error as Error).message).toMatch(/not reachable/i);
      }
      expect(client.status().phase).toBe('app_not_running');
      client.dispose();
    }
  );
});
