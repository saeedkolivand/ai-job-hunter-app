/**
 * Shared harness for the connection-status suites: mounts the component against
 * bare `<div>` hosts and a mocked `send`. The suites own the `vi.mock` calls
 * for `@wxt-dev/browser` (this module's `browser` import resolves to it).
 */

import { vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type { ConnectionStatus, PopupRequest, PopupResponse } from '../../lib/messages';
import { type ConnectionStatusDeps, mountConnectionStatus } from '../connection-status';

export const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

export type Phase = ConnectionStatus['phase'];

/** A `getStatus` reply (also the shape of a live status push). */
export const statusReply = (phase: Phase, hasToken = true) =>
  ({ ok: true, kind: 'status', status: { phase, port: null, hasToken } }) as const;

export const newSend = () => vi.fn<(req: PopupRequest) => Promise<PopupResponse>>();

export function mount(deps: Partial<ConnectionStatusDeps> = {}) {
  const pillHost = document.createElement('div');
  const viewsHost = document.createElement('div');
  const send = deps.send ?? newSend();
  const view = mountConnectionStatus(pillHost, viewsHost, { send, ...deps });
  return { pillHost, viewsHost, send: send as ReturnType<typeof vi.fn>, view };
}

export const byId = <T extends HTMLElement>(host: HTMLElement, id: string) =>
  host.querySelector<T>(`#${id}`) as T;

/** Mount, `start()`, let the first fetch (answering `phase`) land, and hand back
 *  a `push(phase)` that delivers a live status push through the registered listener. */
export async function started(
  phase: Phase = 'searching',
  { hasToken = true, ...deps }: Partial<ConnectionStatusDeps> & { hasToken?: boolean } = {}
) {
  const send = newSend().mockResolvedValue(statusReply(phase, hasToken));
  const mounted = mount({ send, ...deps });
  mounted.view.start();
  await flush();
  const registered = vi.mocked(browser.runtime.onMessage.addListener).mock.calls.at(-1)?.[0];
  if (!registered) throw new Error('onMessage listener not registered');
  const listener = registered as (message: unknown) => void;
  return { ...mounted, listener, push: (next: Phase) => listener(statusReply(next, hasToken)) };
}

export const pillText = (pillHost: HTMLElement): string | null =>
  byId(pillHost, 'status-pill').textContent;
export const retryHidden = (pillHost: HTMLElement): boolean =>
  byId<HTMLButtonElement>(pillHost, 'btn-retry').hidden === true;
