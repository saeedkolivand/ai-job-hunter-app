import { beforeEach, describe, expect, it, vi } from 'vitest';

// Mock the Tauri transport so namespace wrappers can be exercised in node/jsdom.
const invoke = vi.fn().mockResolvedValue(undefined);
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

import { createTauriInvokeClient } from '../index';
import { ai } from './ai';
import { documents } from './documents';
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

    scrape.board({ board: 'linkedin', query: 'react', pages: 1 });
    expect(invoke).toHaveBeenCalledWith('scrape_board', {
      req: { board: 'linkedin', query: 'react', pages: 1 },
    });

    documents.remove('doc-1');
    expect(invoke).toHaveBeenCalledWith('documents_remove', { id: 'doc-1' });

    jobs.get('job-1');
    expect(invoke).toHaveBeenCalledWith('jobs_get', { jobId: 'job-1' });
  });

  it('wires event subscriptions through listen and forwards payloads', () => {
    const handler = vi.fn();
    const unsub = ai.onStream(handler);
    expect(listen).toHaveBeenCalledWith('ai:stream', expect.any(Function));

    // Simulate the backend emitting an event.
    lastListenHandler?.({ payload: { token: 'hi' } });
    expect(handler).toHaveBeenCalledWith({ token: 'hi' });
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
