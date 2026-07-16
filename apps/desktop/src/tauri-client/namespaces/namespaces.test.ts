import { beforeEach, describe, expect, it, vi } from 'vitest';

// Mock the Tauri transport so namespace wrappers can be exercised in node/jsdom.
// Most namespaces tolerate an `undefined` resolve; the few that strictly validate
// their envelope (e.g. `github_import_repos`, which throws on a malformed result
// rather than masking a failure as an empty list) get a channel-shaped reply so
// the generic "exercise every method" sweep doesn't produce an unhandled rejection.
const invoke = vi.fn(async (...args: unknown[]) => {
  // Strict-envelope channels get a channel-shaped reply; everything else resolves
  // undefined (which the tolerant namespaces accept).
  if (args[0] === 'github_import_repos') return { repos: [] };
  return undefined;
});
let lastListenHandler: ((e: { payload: unknown }) => void) | null = null;
const unlisten = vi.fn();
const listen = vi.fn(async (_event: string, handler: (e: { payload: unknown }) => void) => {
  lastListenHandler = handler;
  return unlisten;
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: [string, (e: { payload: unknown }) => void]) => listen(...args),
}));

import { EVENT_CHANNELS } from '@ajh/shared';

import { createTauriInvokeClient } from '../index';
import { agent } from './agent';
import { ai } from './ai';
import { applications } from './applications';
import { boards } from './boards';
import { documents } from './documents';
import { extensionBridge } from './extensionBridge';
import { jobs } from './jobs';
import { scrape } from './scrape';
import { system } from './system';

beforeEach(() => {
  invoke.mockClear();
  listen.mockClear();
  unlisten.mockClear();
  lastListenHandler = null;
});

describe('tauri-client namespaces', () => {
  it('maps method calls to the expected invoke channels', () => {
    system.getVersion();
    expect(invoke).toHaveBeenCalledWith('system_get_version');

    system.setLocale('de');
    expect(invoke).toHaveBeenCalledWith('system_set_locale', { locale: 'de' });

    scrape.boards({ boards: ['linkedin'], query: 'react', amount: 25 });
    expect(invoke).toHaveBeenCalledWith('scrape_boards', {
      req: { boards: ['linkedin'], query: 'react', amount: 25 },
    });

    documents.remove('doc-1');
    expect(invoke).toHaveBeenCalledWith('documents_remove', { id: 'doc-1' });

    jobs.get('job-1');
    expect(invoke).toHaveBeenCalledWith('jobs_get', { jobId: 'job-1' });

    extensionBridge.status();
    expect(invoke).toHaveBeenCalledWith('extension_bridge_status');

    extensionBridge.regenerateToken();
    expect(invoke).toHaveBeenCalledWith('extension_bridge_regenerate_token');

    boards.connect({ boardId: 'indeed' });
    expect(invoke).toHaveBeenCalledWith('boards_login_with_browser', { boardId: 'indeed' });

    const agentReq = { resumeId: 'res-1', jobId: 'job-1' };
    agent.run(agentReq);
    expect(invoke).toHaveBeenCalledWith('agent_run', { req: agentReq });

    boards.disconnect({ boardId: 'indeed' });
    expect(invoke).toHaveBeenCalledWith('boards_logout', { boardId: 'indeed' });
  });

  it('wires event subscriptions through listen and forwards payloads', () => {
    const handler = vi.fn();
    const unsub = ai.onStream(handler);
    expect(listen).toHaveBeenCalledWith(EVENT_CHANNELS.ai.stream, expect.any(Function));

    // Simulate the backend emitting an event.
    lastListenHandler?.({ payload: { token: 'hi' } });
    expect(handler).toHaveBeenCalledWith({ token: 'hi' });
    expect(typeof unsub).toBe('function');
  });

  it('wires agent:step through listen and forwards the payload', () => {
    const handler = vi.fn();
    const unsub = agent.onStep(handler);
    expect(listen).toHaveBeenCalledWith(EVENT_CHANNELS.agent.step, expect.any(Function));

    const step = { jobId: 'job-1', step: 1, text: 'planning', tools: [], denied: [], kind: 'turn' };
    lastListenHandler?.({ payload: step });
    expect(handler).toHaveBeenCalledWith(step);
    expect(typeof unsub).toBe('function');
  });

  it('wires applications:changed through listen and forwards the payload', () => {
    const handler = vi.fn();
    const unsub = applications.onChanged(handler);
    expect(listen).toHaveBeenCalledWith(EVENT_CHANNELS.applications.changed, expect.any(Function));

    // Drive the inner callback so the arrow function body is covered.
    lastListenHandler?.({ payload: { applicationId: 'app-1' } });
    expect(handler).toHaveBeenCalledWith({ applicationId: 'app-1' });
    expect(typeof unsub).toBe('function');
  });

  it('wires extensionBridge:changed through listen and forwards the payload', () => {
    const handler = vi.fn();
    const unsub = extensionBridge.onChanged(handler);
    expect(listen).toHaveBeenCalledWith(
      EVENT_CHANNELS.extensionBridge.changed,
      expect.any(Function)
    );

    lastListenHandler?.({ payload: { connected: true } });
    expect(handler).toHaveBeenCalledWith({ connected: true });
    expect(typeof unsub).toBe('function');
  });

  it('exercises every method across every namespace', () => {
    const client = createTauriInvokeClient() as unknown as Record<string, Record<string, unknown>>;
    let invokeMethods = 0;
    for (const namespace of Object.values(client)) {
      for (const [name, fn] of Object.entries(namespace)) {
        if (typeof fn !== 'function') continue;
        // `cliAgents.install` runs through the shell plugin, not `invoke`, so it
        // neither calls the transport nor counts toward the invoke total (#22).
        if (name === 'install') continue;
        // Event subscriptions take a handler; everything else takes an args object.
        if (name.startsWith('on')) {
          (fn as (h: () => void) => unknown)(() => {});
        } else {
          (fn as (arg: unknown) => unknown)({});
          invokeMethods++;
        }
      }
    }
    expect(invoke).toHaveBeenCalledTimes(invokeMethods);
  });
});
