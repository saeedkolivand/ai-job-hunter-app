import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type { AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';

vi.mock('@wxt-dev/browser', () => ({
  browser: { tabs: { create: vi.fn() }, runtime: { openOptionsPage: vi.fn() } },
}));

import { mountPrep, parsePrepResourceData, prepHasContent } from './prep';

// ---------------------------------------------------------------------------
// parsePrepResourceData / prepHasContent (pure)
// ---------------------------------------------------------------------------

describe('parsePrepResourceData', () => {
  it('parses a full payload', () => {
    const data = parsePrepResourceData({
      generation: {
        hasCompanyBrief: true,
        companyBrief: 'Acme makes widgets.',
        interviewQuestions: [
          { question: 'Tell me about yourself', why: 'Warm-up', audience: 'recruiter' },
          { question: 'Why us?' },
        ],
        salaryAnswer: 'I am targeting $120k-$140k.',
        updatedAt: 123,
      },
    });
    expect(data).toEqual({
      hasCompanyBrief: true,
      companyBrief: 'Acme makes widgets.',
      interviewQuestions: [
        { question: 'Tell me about yourself', why: 'Warm-up', audience: 'recruiter' },
        { question: 'Why us?', why: undefined, audience: undefined },
      ],
      salaryAnswer: 'I am targeting $120k-$140k.',
    });
  });

  it('degrades to empty on malformed/missing data (never throws)', () => {
    const EMPTY = {
      hasCompanyBrief: false,
      companyBrief: null,
      interviewQuestions: [],
      salaryAnswer: null,
    };
    expect(parsePrepResourceData(null)).toEqual(EMPTY);
    expect(parsePrepResourceData(undefined)).toEqual(EMPTY);
    expect(parsePrepResourceData('nope')).toEqual(EMPTY);
    expect(parsePrepResourceData({})).toEqual(EMPTY);
    expect(parsePrepResourceData({ generation: null })).toEqual(EMPTY);
  });

  it('drops a malformed interview-question entry (missing question) without failing the whole list', () => {
    const data = parsePrepResourceData({
      generation: {
        hasCompanyBrief: false,
        interviewQuestions: [{ why: 'no question text' }, { question: 'Good one?' }],
      },
    });
    expect(data.interviewQuestions).toEqual([
      { question: 'Good one?', why: undefined, audience: undefined },
    ]);
  });
});

describe('prepHasContent', () => {
  it('false when the job has nothing yet', () => {
    expect(
      prepHasContent({
        hasCompanyBrief: false,
        companyBrief: null,
        interviewQuestions: [],
        salaryAnswer: null,
      })
    ).toBe(false);
  });

  it('true when only interview questions exist', () => {
    expect(
      prepHasContent({
        hasCompanyBrief: false,
        companyBrief: null,
        interviewQuestions: [{ question: 'Why us?' }],
        salaryAnswer: null,
      })
    ).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// mountPrep (the view)
// ---------------------------------------------------------------------------

function makeDeps(send: (req: PopupRequest) => Promise<PopupResponse>) {
  return { send, copy: vi.fn(async () => true) };
}

const AI_ASSIST_ON: PopupResponse = {
  ok: true,
  kind: 'settingsGet',
  result: {
    ok: true,
    settings: { autofill: true, aiAssist: true, autotrack: false, saveAnswersOnSubmit: false },
  },
};

const AI_ASSIST_OFF: PopupResponse = {
  ok: true,
  kind: 'settingsGet',
  result: {
    ok: true,
    settings: { autofill: true, aiAssist: false, autotrack: false, saveAnswersOnSubmit: false },
  },
};

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

/** Routes a send() call to the right canned response by request kind — the
 *  view fires `prepGet` and `settingsGet` concurrently via `Promise.all`. */
function router(
  byKind: Partial<Record<PopupRequest['kind'], PopupResponse>>
): (req: PopupRequest) => Promise<PopupResponse> {
  return async (req) => byKind[req.kind] ?? { ok: false, error: `unhandled: ${req.kind}` };
}

/** Wait until a NOT-disabled button with this exact text exists — the
 *  draft buttons render disabled until `settings.get` answers, and clicking
 *  a still-disabled button is a browser-spec no-op (jsdom included). */
async function waitForEnabledButton(host: HTMLElement, label: string): Promise<HTMLButtonElement> {
  return vi.waitFor(() => {
    const btn = Array.from(host.querySelectorAll<HTMLButtonElement>('button')).find(
      (b) => b.textContent === label && !b.disabled
    );
    if (!btn) throw new Error(`no enabled button "${label}" yet`);
    return btn;
  });
}

describe('mountPrep', () => {
  let host: HTMLElement;

  beforeEach(() => {
    document.body.innerHTML = '<div id="host"></div>';
    host = document.getElementById('host')!;
    vi.mocked(browser.tabs.create).mockClear();
    vi.mocked(browser.runtime.openOptionsPage).mockClear();
  });

  it('shows a loading state before refresh() resolves', () => {
    const deps = makeDeps(() => new Promise<PopupResponse>(() => {}));
    mountPrep(host, deps);
    expect(host.textContent).toContain('Loading…');
  });

  it('nothing yet: shows "Prepare in the app" once the url resolves, opening the deep link on click', async () => {
    const send = vi.fn(
      router({
        prepGet: prepResult({ hasCompanyBrief: false, interviewQuestions: [] }),
        settingsGet: AI_ASSIST_ON,
      })
    );
    const view = mountPrep(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Prepare in the app'));

    const link = host.querySelector<HTMLButtonElement>('button.btn--quiet');
    link?.click();
    await vi.waitFor(() =>
      expect(browser.tabs.create).toHaveBeenCalledWith({
        url: 'ajh://prep?url=https%3A%2F%2Fexample.com%2Fjob%2F1',
      })
    );
  });

  it('everything: renders the company brief, interview questions and salary answer', async () => {
    const send = vi.fn(
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
    const view = mountPrep(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Acme makes widgets.'));

    expect(host.textContent).toContain('Interview questions (1)');
    expect(host.textContent).toContain('Why us?');
    expect(host.textContent).toContain('Targeting $120k.');
  });

  it('brief only: interview questions and salary each fall back to their own draft button', async () => {
    const send = vi.fn(
      router({
        prepGet: prepResult({
          hasCompanyBrief: true,
          companyBrief: 'Acme makes widgets.',
          interviewQuestions: [],
        }),
        settingsGet: AI_ASSIST_ON,
      })
    );
    const view = mountPrep(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Acme makes widgets.'));

    expect(host.textContent).not.toContain('Interview questions');
    expect(host.querySelector('button')?.textContent).not.toBe(null);
    expect(host.textContent).toContain('Draft salary answer');
  });

  it('renders the desktop refusal verbatim', async () => {
    const send = vi.fn(
      router({ prepGet: prepRefusal('Assisted autofill is off.'), settingsGet: AI_ASSIST_ON })
    );
    const view = mountPrep(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Assisted autofill is off.'));
  });

  it('AI-assist off: each draft button is replaced with an explanation + a link to Settings, never firing the request', async () => {
    const send = vi.fn(
      router({
        prepGet: prepResult({ hasCompanyBrief: false, interviewQuestions: [] }),
        settingsGet: AI_ASSIST_OFF,
      })
    );
    const view = mountPrep(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('AI-answer-assist is off.'));

    send.mockClear();
    const settingsLink = Array.from(host.querySelectorAll<HTMLButtonElement>('button')).find(
      (b) => b.textContent === 'Turn on in Settings'
    );
    settingsLink?.click();
    expect(browser.runtime.openOptionsPage).toHaveBeenCalled();
    expect(send).not.toHaveBeenCalled();
  });

  it('drafting: click fires answer.assist with the topic, and a completed draft renders copy-ready with no leftover button', async () => {
    let resolveAssist: ((res: PopupResponse) => void) | undefined;
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind === 'prepGet')
        return prepResult({ hasCompanyBrief: false, interviewQuestions: [] });
      if (req.kind === 'settingsGet') return AI_ASSIST_ON;
      if (req.kind === 'answerAssist') {
        return new Promise<PopupResponse>((resolve) => {
          resolveAssist = resolve;
        });
      }
      return { ok: false, error: `unhandled: ${req.kind}` };
    });
    const view = mountPrep(host, makeDeps(send));
    view.refresh();
    const draftBtn = await waitForEnabledButton(host, 'Draft company brief');
    draftBtn.click();
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
    expect(
      Array.from(host.querySelectorAll('button')).some((b) => b.textContent === 'Cancel')
    ).toBe(false);
  });

  it('drafting one topic disables the OTHER draft button so a second click cannot silently supersede the first', async () => {
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind === 'prepGet')
        return prepResult({ hasCompanyBrief: false, interviewQuestions: [] });
      if (req.kind === 'settingsGet') return AI_ASSIST_ON;
      if (req.kind === 'answerAssist') return new Promise<PopupResponse>(() => {});
      return { ok: false, error: `unhandled: ${req.kind}` };
    });
    const view = mountPrep(host, makeDeps(send));
    view.refresh();
    const briefBtn = await waitForEnabledButton(host, 'Draft company brief');
    briefBtn.click();
    await vi.waitFor(() => expect(host.textContent).toContain('Cancel'));

    const salaryBtn = Array.from(host.querySelectorAll<HTMLButtonElement>('button')).find(
      (b) => b.textContent === 'Draft salary answer'
    );
    expect(salaryBtn?.disabled).toBe(true);
    salaryBtn?.click();
    // A disabled button never fires its click handler — no second request.
    expect(send).not.toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'answerAssist', topic: 'salary-answer' })
    );
  });

  it('cancel sends assistCancel and clears the pending state', async () => {
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind === 'prepGet')
        return prepResult({ hasCompanyBrief: false, interviewQuestions: [] });
      if (req.kind === 'settingsGet') return AI_ASSIST_ON;
      if (req.kind === 'answerAssist') return new Promise<PopupResponse>(() => {});
      if (req.kind === 'assistCancel') return { ok: true, kind: 'assistCancel' };
      return { ok: false, error: `unhandled: ${req.kind}` };
    });
    const view = mountPrep(host, makeDeps(send));
    view.refresh();
    const draftBtn = await waitForEnabledButton(host, 'Draft company brief');
    draftBtn.click();
    await vi.waitFor(() => expect(host.textContent).toContain('Cancel'));

    const cancelBtn = Array.from(host.querySelectorAll<HTMLButtonElement>('button')).find(
      (b) => b.textContent === 'Cancel'
    );
    cancelBtn?.click();
    await vi.waitFor(() => expect(send).toHaveBeenCalledWith({ kind: 'assistCancel' }));
    expect(host.textContent).toContain('Draft company brief');
  });

  it('reset() clears data and any pending draft', async () => {
    const send = vi.fn(
      router({
        prepGet: prepResult({
          hasCompanyBrief: true,
          companyBrief: 'Acme.',
          interviewQuestions: [],
        }),
        settingsGet: AI_ASSIST_ON,
      })
    );
    const view = mountPrep(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Acme.'));

    view.reset();
    expect(host.textContent).not.toContain('Acme.');
  });
});
