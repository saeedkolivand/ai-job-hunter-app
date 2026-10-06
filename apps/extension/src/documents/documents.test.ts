import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type { AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';

vi.mock('@wxt-dev/browser', () => ({
  browser: { tabs: { create: vi.fn() } },
}));

import { JOB_TOOLS_GATED_LINE } from '../job-tools/job-tools';
import { mountDocuments } from './documents';

// ---------------------------------------------------------------------------
// mountDocuments (the view)
// ---------------------------------------------------------------------------

const JOB_URL = 'https://example.com/job/1';
const ATTACH = 'Attach résumé to this page';

/** A `documentsList` reply carrying `data` for the job at {@link JOB_URL}. */
const listOf = (data: unknown): PopupResponse => ({
  ok: true,
  kind: 'documentsList',
  result: { ok: true, resource: 'documents', data },
  url: JOB_URL,
});

const GENERATION_RESULT = listOf({
  generation: { hasResume: true, hasCoverLetter: true, jobTitle: 'Engineer', company: 'Acme' },
  documents: [],
});

const LETTER_TEXT: PopupResponse = {
  ok: true,
  kind: 'documentExportText',
  text: 'Dear hiring manager…',
  filename: 'letter.txt',
};

/** Answers `documentsList` with the generation, plus any extra `handlers` by request
 *  kind; anything else fails the test loudly. */
const routed = (handlers: Partial<Record<PopupRequest['kind'], PopupResponse>> = {}) =>
  vi.fn(async (req: PopupRequest): Promise<PopupResponse> => {
    if (req.kind === 'documentsList') return GENERATION_RESULT;
    const reply = handlers[req.kind];
    if (reply) return reply;
    throw new Error(`unexpected request ${req.kind}`);
  });

/** One empty "Cover letter" field the paste picker can offer. */
const COVER_LETTER_STATE = {
  tabId: 1,
  origin: 'https://example.com',
  scannedAt: 1,
  pageChanged: false,
  stream: null,
  rows: [
    {
      id: 'empty:0:Cover letter',
      question: 'Cover letter',
      status: 'empty',
      versions: [],
      selected: -1,
      field: { kind: 'empty', index: 0, count: 1, currentText: '', originalText: '' },
    },
  ],
} satisfies AnswerState;

describe('mountDocuments', () => {
  let host: HTMLElement;

  beforeEach(() => {
    document.body.innerHTML = '<div id="host"></div>';
    host = document.getElementById('host')!;
  });

  function makeDeps(
    send: (req: PopupRequest) => Promise<PopupResponse>,
    getFollowGeneration: () => number = () => 0
  ) {
    return {
      send,
      copy: vi.fn(async () => true),
      confirmAttach: vi.fn(async () => true),
      currentHost: () => 'example.com',
      getFollowGeneration,
      onUrlResolved: vi.fn(),
    };
  }

  /** Mount against `send` and kick off the first refresh. */
  function mountRefreshed(
    send: (req: PopupRequest) => Promise<PopupResponse>,
    deps = makeDeps(send)
  ) {
    const view = mountDocuments(host, deps);
    view.refresh();
    return { view, deps };
  }

  const click = (label: string): void =>
    Array.from(host.querySelectorAll('button'))
      .find((b) => b.textContent === label)!
      .click();

  /** Refresh against the generation, then wait for the résumé view to render. */
  async function ready(send = routed(), deps = makeDeps(send)) {
    const mounted = mountRefreshed(send, deps);
    await vi.waitFor(() => expect(host.textContent).toContain(ATTACH));
    return { ...mounted, send };
  }

  /** From a refreshed view: feed the cover-letter field, open the paste picker and
   *  click its one row. */
  async function pasteIntoCoverLetter(view: ReturnType<typeof mountDocuments>) {
    view.render(COVER_LETTER_STATE);
    click('Cover letter');
    await vi.waitFor(() => expect(host.textContent).toContain('Paste cover letter…'));
    click('Paste cover letter…');
    await vi.waitFor(() => expect(host.textContent).toContain('Cover letter'));
    host.querySelector<HTMLButtonElement>('.picker__row')!.click();
  }

  it('mounts showing nothing, and shows "Loading…" only while refresh() is genuinely in flight (#1225)', () => {
    const view = mountDocuments(host, makeDeps(vi.fn(() => new Promise<PopupResponse>(() => {}))));
    // The mount alone is NOT a fetch — no phantom "Loading…" (#1225).
    expect(host.textContent).not.toContain('Loading…');
    expect(host.textContent).toBe('');

    view.refresh();
    // Only now, with an unanswered request in flight, does the empty state
    // say "Loading…".
    expect(host.textContent).toContain('Loading…');
  });

  it('refresh() renders the résumé kind by default once a candidate resolves', async () => {
    const { deps } = mountRefreshed(routed());
    await vi.waitFor(() => expect(host.querySelector('select')).not.toBeNull());

    expect(host.textContent).toContain(ATTACH);
    expect(deps.onUrlResolved).toHaveBeenCalledWith(JOB_URL);
  });

  it('shows the empty state with a "Generate in the app" deep-link button that opens a tab on click', async () => {
    mountRefreshed(async () => listOf({ generation: null, documents: [] }));
    await vi.waitFor(() => expect(host.textContent).toContain('No documents yet'));

    // A button (not a raw `ajh://` anchor) — mirrors the extension's
    // already-verified `browser.tabs.create` deep-link trigger.
    const link = host.querySelector<HTMLButtonElement>('button.btn--quiet');
    expect(link?.textContent).toBe('Generate in the app');
    link?.click();
    await vi.waitFor(() =>
      expect(browser.tabs.create).toHaveBeenCalledWith({
        url: 'ajh://generate?url=https%3A%2F%2Fexample.com%2Fjob%2F1',
      })
    );
  });

  it('renders the desktop refusal verbatim and never shows the generate link for it', async () => {
    mountRefreshed(async () => ({
      ok: true,
      kind: 'documentsList',
      result: { ok: false, resource: 'documents', error: 'Assisted autofill is off.' },
      url: JOB_URL,
    }));
    await vi.waitFor(() => expect(host.textContent).toContain('Assisted autofill is off.'));
    expect(host.querySelector('button.btn--quiet')).toBeNull();
  });

  it('disables the Cover letter toggle when the picked candidate has none', async () => {
    mountRefreshed(async () =>
      listOf({
        generation: { hasResume: true, hasCoverLetter: false, jobTitle: 'Engineer' },
        documents: [],
      })
    );
    await vi.waitFor(() => expect(host.querySelector('select')).not.toBeNull());

    const letterBtn = Array.from(host.querySelectorAll('button')).find(
      (b) => b.textContent === 'Cover letter'
    );
    expect(letterBtn?.disabled).toBe(true);
  });

  it('Attach: asks confirmAttach first, then sends documentAttach and shows the result', async () => {
    const send = routed({
      documentAttach: {
        ok: true,
        kind: 'documentAttach',
        result: { attached: true, filename: 'resume.pdf', byteLength: 100 },
      },
    });
    const { deps } = await ready(send);

    click(ATTACH);

    await vi.waitFor(() => expect(host.textContent).toContain('Attached resume.pdf'));
    expect(deps.confirmAttach).toHaveBeenCalledWith('example.com');
    expect(send).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'documentAttach', templateId: 'classic', format: 'pdf' })
    );
  });

  it('Attach: never sends the request when confirmAttach resolves false', async () => {
    const send = routed();
    const deps = makeDeps(send);
    deps.confirmAttach = vi.fn(async () => false);
    await ready(send, deps);

    click(ATTACH);
    await Promise.resolve();
    await Promise.resolve();

    expect(send).not.toHaveBeenCalledWith(expect.objectContaining({ kind: 'documentAttach' }));
  });

  it('Copy cover letter: fetches the text and copies it', async () => {
    const { deps } = await ready(routed({ documentExportText: LETTER_TEXT }));

    click('Cover letter');
    await vi.waitFor(() => expect(host.textContent).toContain('Copy cover letter'));
    click('Copy cover letter');

    await vi.waitFor(() => expect(deps.copy).toHaveBeenCalledWith('Dear hiring manager…'));
    await vi.waitFor(() => expect(host.textContent).toContain('Copied.'));
  });

  it('Paste cover letter: opens a field picker from the fed AnswerState and dispatches answerFill for an empty field', async () => {
    const send = routed({
      documentExportText: LETTER_TEXT,
      answerFill: { ok: true, kind: 'answerFill', result: { filled: true } },
    });
    const { view } = await ready(send);

    await pasteIntoCoverLetter(view);

    await vi.waitFor(() =>
      expect(send).toHaveBeenCalledWith(
        expect.objectContaining({
          kind: 'answerFill',
          question: 'Cover letter',
          answer: 'Dear hiring manager…',
        })
      )
    );
    await vi.waitFor(() => expect(host.textContent).toContain('Pasted into the field.'));
  });

  it('Paste cover letter: aborts the send when the followed tab changes during the export wait', async () => {
    let resolveExport: ((res: PopupResponse) => void) | undefined;
    const base = routed();
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind !== 'documentExportText') return base(req);
      return new Promise<PopupResponse>((resolve) => {
        resolveExport = resolve;
      });
    });
    let followGeneration = 0;
    const { view } = await ready(
      send,
      makeDeps(send, () => followGeneration)
    );

    await pasteIntoCoverLetter(view);
    await vi.waitFor(() =>
      expect(send).toHaveBeenCalledWith(expect.objectContaining({ kind: 'documentExportText' }))
    );

    // The panel followed a different tab while the export above was still
    // in flight — the stale paste must never fire.
    followGeneration += 1;
    resolveExport?.(LETTER_TEXT);

    await vi.waitFor(() => expect(host.textContent).toContain('The followed tab changed'));
    expect(send).not.toHaveBeenCalledWith(expect.objectContaining({ kind: 'answerFill' }));
  });

  it('reset() clears candidates and any open picker, leaving no phantom "Loading…" (#1225)', async () => {
    const { view } = await ready();

    view.reset();
    expect(host.textContent).not.toContain('Loading…');
    expect(host.textContent).not.toContain(ATTACH);
  });

  it('reset(reason) renders the caller line and never a deep link (#1225)', async () => {
    const { view } = await ready();

    // sidepanel.ts resets with the SHARED gated line on an untrusted tab.
    view.reset(JOB_TOOLS_GATED_LINE);
    expect(host.textContent).toContain(JOB_TOOLS_GATED_LINE);
    // A gated reset is untrusted — `lastUrl` is cleared, so the "Generate in
    // the app" deep link must never appear alongside the line.
    expect(host.querySelector('button.btn--quiet')).toBeNull();
  });

  it('kind-mismatch: surfaces an error instead of sitting on a phantom "Loading…" (#1225)', async () => {
    mountRefreshed(async () => ({ ok: true, kind: 'appliedCheck', result: { found: false } }));
    await vi.waitFor(() =>
      expect(host.textContent).toContain('Unexpected response — please retry.')
    );
    expect(host.textContent).not.toContain('Loading…');
  });
});
