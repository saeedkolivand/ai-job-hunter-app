/**
 * Shared harness for the job-tools suites: mount the component into a bare
 * `<div>` host against a mocked `send`, without going through either popup.ts
 * or sidepanel.ts.
 */

import { type Mock, vi } from 'vitest';

import type { AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import { type JobToolsDeps, type JobToolsView, mountJobTools } from './job-tools';

export const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

export type Send = (req: PopupRequest) => Promise<PopupResponse>;

export function answerState(over: Partial<AnswerState> = {}): AnswerState {
  return {
    tabId: 1,
    origin: 'https://jobs.example.com',
    scannedAt: 0,
    rows: [],
    stream: null,
    pageChanged: false,
    ...over,
  };
}

/** Mount against `sendImpl` (default: every request fails "not configured"). A
 *  `vi.fn` passed in is used as-is so its own call history stays assertable. */
export function mount(
  sendImpl?: Send,
  extraDeps: Partial<JobToolsDeps> = {}
): {
  host: HTMLDivElement;
  send: Mock<Send>;
  onAnswerToolsVisibility: Mock;
  view: JobToolsView;
} {
  const host = document.createElement('div');
  const fallback: Send = async () => ({ ok: false, error: 'not configured' });
  const send = (vi.isMockFunction(sendImpl) ? sendImpl : vi.fn(sendImpl ?? fallback)) as Mock<Send>;
  const onAnswerToolsVisibility = vi.fn();
  const view = mountJobTools(host, { send, onAnswerToolsVisibility, ...extraDeps });
  return { host, send, onAnswerToolsVisibility, view };
}

export const msg = (host: HTMLElement) =>
  host.querySelector('#job-tools-msg') as HTMLParagraphElement;

/** `host.querySelector` typed as a button — the controls are all `#btn-…`. */
export const btn = (host: HTMLElement, selector: string) =>
  host.querySelector(selector) as HTMLButtonElement;

/** The `fill` reply for a page where nothing matched (also triggers the profile fallback). */
export const FILLED_NOTHING: PopupResponse = {
  ok: true,
  kind: 'fill',
  summary: { filled: [], nameSplit: null, filledNothing: true },
};

/** A `send` that answers `fill` with `fillReply` and `profileGet` with `profile`
 *  (when given); anything else is refused. */
export const fillRouter =
  (fillReply: PopupResponse, profile?: PopupResponse): Send =>
  async (req) => {
    if (req.kind === 'fill') return fillReply;
    if (req.kind === 'profileGet' && profile) return profile;
    return { ok: false, error: 'unused' };
  };

/** A successful reply of `kind` carrying `rest` (typed loosely — these are fixtures). */
export const reply = (kind: string, rest: Record<string, unknown> = {}) =>
  ({ ok: true as const, kind, ...rest }) as never;
export const failure = (error: string) => ({ ok: false as const, error });
