import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type { AnswerState } from '../../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../../lib/messages';

vi.mock('@wxt-dev/browser', () => ({
  browser: { tabs: { create: vi.fn() }, runtime: { openOptionsPage: vi.fn() } },
}));

import { JOB_TOOLS_GATED_LINE } from '../../job-tools/job-tools';
import { mountPrep } from '../prep';

// ---------------------------------------------------------------------------
// mountPrep (the view)
// ---------------------------------------------------------------------------

const settings = (aiAssist: boolean): PopupResponse => ({
  ok: true,
  kind: 'settingsGet',
  result: {
    ok: true,
    settings: { autofill: true, aiAssist, autotrack: false, saveAnswersOnSubmit: false },
  },
});
const AI_ASSIST_ON = settings(true);
const AI_ASSIST_OFF = settings(false);

/** `generation` (the wire's nesting, mirrors `documents`' own resource shape
 *  — see the Rust doc for `agent_read/prep.rs`) built from the flat fields
 *  this file's callers pass, so each call site reads like the pure-parser
 *  tests above rather than repeating the wire's own nesting everywhere. */
function prepResult(generation: unknown, url = 'https://example.com/job/1'): PopupResponse {
  return {
    ok: true,
    kind: 'prepGet',
    result: { ok: true, resource: 'prep', data: { generation } },
    url,
  };
}

function prepRefusal(error: string, url = 'https://example.com/job/1'): PopupResponse {
  return { ok: true, kind: 'prepGet', result: { ok: false, resource: 'prep', error }, url };
}

/** A fresh job: nothing generated yet. */
const NOTHING_YET = prepResult({ hasCompanyBrief: false, interviewQuestions: [] });

const unhandled = (req: PopupRequest): PopupResponse => ({
  ok: false,
  error: `unhandled: ${req.kind}`,
});

/** Routes a send() call to the right canned response by request kind — the
 *  view fires `prepGet` and `settingsGet` concurrently via `Promise.all`. */
function router(
  byKind: Partial<Record<PopupRequest['kind'], PopupResponse>>
): (req: PopupRequest) => Promise<PopupResponse> {
  return async (req) => byKind[req.kind] ?? unhandled(req);
}

/** A fresh job with AI assist ON whose `answerAssist` never settles (a draft
 *  stuck in flight) — `assistCancel` is acknowledged. */
const draftInFlight = () =>
  vi.fn(async (req: PopupRequest): Promise<PopupResponse> => {
    if (req.kind === 'prepGet') return NOTHING_YET;
    if (req.kind === 'settingsGet') return AI_ASSIST_ON;
    if (req.kind === 'answerAssist') return new Promise<PopupResponse>(() => {});
    if (req.kind === 'assistCancel') return { ok: true, kind: 'assistCancel' };
    return unhandled(req);
  });

/** Wait until a NOT-disabled button with this exact text exists — the
 *  draft buttons render disabled until `settings.get` answers, and clicking
 *  a still-disabled button is a browser-spec no-op (jsdom included). */
async function waitForEnabledButton(host: HTMLElement, label: string): Promise<HTMLButtonElement> {
  return vi.waitFor(() => {
    const btn = buttonByText(host, label);
    if (!btn || btn.disabled) throw new Error(`no enabled button "${label}" yet`);
    return btn;
  });
}

const buttonByText = (host: HTMLElement, label: string): HTMLButtonElement | undefined =>
  Array.from(host.querySelectorAll<HTMLButtonElement>('button')).find(
    (b) => b.textContent === label
  );

describe('mountPrep', () => {
  let host: HTMLElement;

  beforeEach(() => {
    document.body.innerHTML = '<div id="host"></div>';
    host = document.getElementById('host')!;
    vi.mocked(browser.tabs.create).mockClear();
    vi.mocked(browser.runtime.openOptionsPage).mockClear();
  });

  /** Mount against `send` and kick off the first refresh. */
  function mountRefreshed(send: (req: PopupRequest) => Promise<PopupResponse>) {
    const view = mountPrep(host, { send, copy: vi.fn(async () => true) });
    view.refresh();
    return view;
  }

  /** Mount a fresh job with AI assist ON whose draft is stuck, click "Draft company brief". */
  async function startBriefDraft(send: ReturnType<typeof draftInFlight>) {
    const view = mountRefreshed(send);
    (await waitForEnabledButton(host, 'Draft company brief')).click();
    await vi.waitFor(() => expect(host.textContent).toContain('Cancel'));
    return view;
  }

  it('mounts showing nothing, and shows "Loading…" only while refresh() is genuinely in flight (#1225)', () => {
    const view = mountPrep(host, {
      send: () => new Promise<PopupResponse>(() => {}),
      copy: vi.fn(async () => true),
    });
    // The mount alone is NOT a fetch — no phantom "Loading…" (#1225).
    expect(host.textContent).not.toContain('Loading…');
    expect(host.textContent).not.toContain('Prepare in the app');
    // The draft buttons render at mount (an empty job still offers them), but
    // DISABLED — nothing has declared the page readable yet (#1234).
    const draftBtns = Array.from(host.querySelectorAll<HTMLButtonElement>('button'));
    expect(draftBtns.some((b) => b.textContent === 'Draft company brief')).toBe(true);
    expect(draftBtns.some((b) => b.textContent === 'Draft salary answer')).toBe(true);
    expect(draftBtns.every((b) => b.disabled)).toBe(true);

    view.refresh();
    // Only now, with an unanswered request in flight, does the empty state
    // say "Loading…".
    expect(host.textContent).toContain('Loading…');
  });

  it('nothing yet: shows "Prepare in the app" once the url resolves, opening the deep link on click', async () => {
    mountRefreshed(vi.fn(router({ prepGet: NOTHING_YET, settingsGet: AI_ASSIST_ON })));
    await vi.waitFor(() => expect(host.textContent).toContain('Prepare in the app'));

    host.querySelector<HTMLButtonElement>('button.btn--quiet')?.click();
    await vi.waitFor(() =>
      expect(browser.tabs.create).toHaveBeenCalledWith({
        url: 'ajh://prep?url=https%3A%2F%2Fexample.com%2Fjob%2F1',
      })
    );
  });

  it('everything: renders the company brief, interview questions and salary answer', async () => {
    mountRefreshed(
      router({
        prepGet: prepResult({
          hasCompanyBrief: true,
          companyBrief: 'Acme makes widgets.',
          interviewQuestions: [{ question: 'Why us?', why: 'Warm-up' }],
          salaryAnswer: 'Targeting $120k.',
        }),
        settingsGet: AI_ASSIST_ON,
      })
    );
    await vi.waitFor(() => expect(host.textContent).toContain('Acme makes widgets.'));

    expect(host.textContent).toContain('Interview questions (1)');
    expect(host.textContent).toContain('Why us?');
    expect(host.textContent).toContain('Targeting $120k.');
  });

  it('brief only: interview questions and salary each fall back to their own draft button', async () => {
    mountRefreshed(
      router({
        prepGet: prepResult({
          hasCompanyBrief: true,
          companyBrief: 'Acme makes widgets.',
          interviewQuestions: [],
        }),
        settingsGet: AI_ASSIST_ON,
      })
    );
    await vi.waitFor(() => expect(host.textContent).toContain('Acme makes widgets.'));

    expect(host.textContent).not.toContain('Interview questions');
    expect(host.querySelector('button')?.textContent).not.toBe(null);
    expect(host.textContent).toContain('Draft salary answer');
  });

  it('renders the desktop refusal verbatim', async () => {
    mountRefreshed(
      router({ prepGet: prepRefusal('Assisted autofill is off.'), settingsGet: AI_ASSIST_ON })
    );
    await vi.waitFor(() => expect(host.textContent).toContain('Assisted autofill is off.'));
  });

  it('AI-assist off: each draft button is replaced with an explanation + a link to Settings, never firing the request', async () => {
    const send = vi.fn(router({ prepGet: NOTHING_YET, settingsGet: AI_ASSIST_OFF }));
    mountRefreshed(send);
    await vi.waitFor(() => expect(host.textContent).toContain('AI-answer-assist is off.'));

    send.mockClear();
    buttonByText(host, 'Turn on in Settings')?.click();
    expect(browser.runtime.openOptionsPage).toHaveBeenCalled();
    expect(send).not.toHaveBeenCalled();
  });

  it('drafting: click fires answer.assist with the topic, and a completed draft renders copy-ready with no leftover button', async () => {
    let resolveAssist: ((res: PopupResponse) => void) | undefined;
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind === 'answerAssist') {
        return new Promise<PopupResponse>((resolve) => {
          resolveAssist = resolve;
        });
      }
      return router({ prepGet: NOTHING_YET, settingsGet: AI_ASSIST_ON })(req);
    });
    const view = mountRefreshed(send);
    (await waitForEnabledButton(host, 'Draft company brief')).click();
    await vi.waitFor(() =>
      expect(send).toHaveBeenCalledWith({
        kind: 'answerAssist',
        question: 'Company brief',
        searchWeb: false,
        topic: 'company-brief',
      })
    );

    // Mid-stream: a state push tagged with this topic renders the live text.
    view.render({
      tabId: 1,
      origin: 'https://example.com',
      scannedAt: 0,
      rows: [],
      stream: {
        rowId: '',
        text: 'Acme make',
        done: false,
        interrupted: false,
        kind: 'draft',
        topic: 'company-brief',
      },
      pageChanged: false,
    } as AnswerState);
    expect(host.textContent).toContain('Acme make');
    expect(host.querySelector('button')?.textContent).toBeDefined();

    resolveAssist?.({
      ok: true,
      kind: 'answerAssist',
      result: {
        ok: true,
        question: 'Company brief',
        draft: 'Acme makes widgets.',
        sourced: { web: false, brief: false, salary: false },
      },
    });
    await vi.waitFor(() => expect(host.textContent).toContain('Acme makes widgets.'));
    // Once finished the draft-button/streaming controls are replaced by the
    // copyable section — no dangling "Cancel"/"Draft…" control for this topic.
    expect(buttonByText(host, 'Cancel')).toBeUndefined();
  });

  it('drafting one topic disables the OTHER draft button so a second click cannot silently supersede the first', async () => {
    const send = draftInFlight();
    await startBriefDraft(send);

    const salaryBtn = buttonByText(host, 'Draft salary answer');
    expect(salaryBtn?.disabled).toBe(true);
    salaryBtn?.click();
    // A disabled button never fires its click handler — no second request.
    expect(send).not.toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'answerAssist', topic: 'salary-answer' })
    );
  });

  it('cancel sends assistCancel and clears the pending state', async () => {
    const send = draftInFlight();
    await startBriefDraft(send);

    buttonByText(host, 'Cancel')?.click();
    await vi.waitFor(() => expect(send).toHaveBeenCalledWith({ kind: 'assistCancel' }));
    expect(host.textContent).toContain('Draft company brief');
  });

  it('reset() clears data and any pending draft', async () => {
    const view = mountRefreshed(
      router({
        prepGet: prepResult({
          hasCompanyBrief: true,
          companyBrief: 'Acme.',
          interviewQuestions: [],
        }),
        settingsGet: AI_ASSIST_ON,
      })
    );
    await vi.waitFor(() => expect(host.textContent).toContain('Acme.'));

    view.reset();
    expect(host.textContent).not.toContain('Acme.');
  });

  it('reset() during a stream cancels the in-flight draft instead of leaving it billing (PR-1209)', async () => {
    const send = draftInFlight();
    const view = await startBriefDraft(send);

    view.reset();
    await vi.waitFor(() => expect(send).toHaveBeenCalledWith({ kind: 'assistCancel' }));
  });

  it('reset() with no draft in flight never sends assistCancel', async () => {
    const send = vi.fn(router({ prepGet: NOTHING_YET, settingsGet: AI_ASSIST_ON }));
    const view = mountRefreshed(send);
    await vi.waitFor(() => expect(host.textContent).toContain('Draft company brief'));

    send.mockClear();
    view.reset();
    expect(send).not.toHaveBeenCalledWith({ kind: 'assistCancel' });
  });

  it('reset(reason) renders the shared gated line with BOTH draft buttons disabled (#1225, #1234)', async () => {
    const view = mountRefreshed(router({ prepGet: NOTHING_YET, settingsGet: AI_ASSIST_ON }));
    // AI assist is ON and the page is readable — both drafts are armed.
    const briefBtn = await waitForEnabledButton(host, 'Draft company brief');
    const salaryBtn = await waitForEnabledButton(host, 'Draft salary answer');
    expect(briefBtn.disabled).toBe(false);
    expect(salaryBtn.disabled).toBe(false);

    // sidepanel.ts resets with the SHARED gated line on an untrusted tab —
    // the page is no longer readable, so the drafts must disable (#1234) and
    // the line renders instead of a phantom "Loading…" (#1225).
    view.reset(JOB_TOOLS_GATED_LINE);
    expect(host.textContent).toContain(JOB_TOOLS_GATED_LINE);
    expect(host.textContent).not.toContain('Loading…');
    expect(buttonByText(host, 'Draft company brief')?.disabled).toBe(true);
    expect(buttonByText(host, 'Draft salary answer')?.disabled).toBe(true);
    // A gated reset is untrusted — `lastUrl` is cleared, so the "Prepare in
    // the app" deep link must never appear alongside the line.
    expect(host.querySelector('button.btn--quiet')).toBeNull();
  });

  it('kind-mismatch: surfaces an error instead of sitting on a phantom "Loading…" (#1225)', async () => {
    mountRefreshed(
      router({
        prepGet: { ok: true, kind: 'appliedCheck', result: { found: false } },
        settingsGet: AI_ASSIST_ON,
      })
    );
    await vi.waitFor(() =>
      expect(host.textContent).toContain('Unexpected response — please retry.')
    );
    expect(host.textContent).not.toContain('Loading…');
  });
});
