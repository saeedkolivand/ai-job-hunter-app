import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type { AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';

vi.mock('@wxt-dev/browser', () => ({
  browser: { tabs: { create: vi.fn() } },
}));

import { JOB_TOOLS_GATED_LINE } from '../job-tools/job-tools';
import { buildCandidates, mountDocuments, parseDocumentsResourceData } from './documents';

// ---------------------------------------------------------------------------
// parseDocumentsResourceData / buildCandidates (pure)
// ---------------------------------------------------------------------------

describe('parseDocumentsResourceData', () => {
  it('parses a full payload', () => {
    const data = parseDocumentsResourceData({
      generation: {
        hasResume: true,
        hasCoverLetter: true,
        jobTitle: 'Backend Engineer',
        company: 'Acme',
        targetLanguage: 'en',
        updatedAt: 123,
      },
      documents: [
        { id: 'doc-1', name: 'My Résumé.pdf', updatedAt: 1 },
        { id: 'doc-2', name: 'Old Résumé.pdf' },
      ],
    });
    expect(data.generation).toEqual({
      hasResume: true,
      hasCoverLetter: true,
      jobTitle: 'Backend Engineer',
      company: 'Acme',
    });
    expect(data.documents).toEqual([
      { id: 'doc-1', name: 'My Résumé.pdf' },
      { id: 'doc-2', name: 'Old Résumé.pdf' },
    ]);
  });

  it('degrades to empty on malformed/missing data (never throws)', () => {
    expect(parseDocumentsResourceData(null)).toEqual({ generation: null, documents: [] });
    expect(parseDocumentsResourceData(undefined)).toEqual({ generation: null, documents: [] });
    expect(parseDocumentsResourceData('nope')).toEqual({ generation: null, documents: [] });
    expect(parseDocumentsResourceData({})).toEqual({ generation: null, documents: [] });
  });

  it('drops a generation object missing the required booleans', () => {
    const data = parseDocumentsResourceData({ generation: { hasResume: true }, documents: [] });
    expect(data.generation).toBeNull();
  });

  it('drops a malformed document entry (missing name) without failing the whole list', () => {
    const data = parseDocumentsResourceData({
      generation: null,
      documents: [{ id: 'doc-1' }, { id: 'doc-2', name: 'Good.pdf' }],
    });
    expect(data.documents).toEqual([{ id: 'doc-2', name: 'Good.pdf' }]);
  });
});

describe('buildCandidates', () => {
  const URL = 'https://example.com/job/1';

  it('returns empty when there is no generation and no documents', () => {
    expect(buildCandidates({ generation: null, documents: [] }, URL)).toEqual([]);
  });

  it('puts the generation candidate first, labelled by title + company', () => {
    const result = buildCandidates(
      {
        generation: {
          hasResume: true,
          hasCoverLetter: true,
          jobTitle: 'Engineer',
          company: 'Acme',
        },
        documents: [{ id: 'doc-1', name: 'Base.pdf' }],
      },
      URL
    );
    expect(result).toEqual([
      { source: { kind: 'generation', url: URL }, label: 'Engineer · Acme', hasCoverLetter: true },
      { source: { kind: 'document', id: 'doc-1' }, label: 'Base.pdf', hasCoverLetter: false },
    ]);
  });

  it('omits the generation candidate when it has no résumé at all', () => {
    const result = buildCandidates(
      { generation: { hasResume: false, hasCoverLetter: false }, documents: [] },
      URL
    );
    expect(result).toEqual([]);
  });

  it('falls back to "This job" when the generation has neither title nor company', () => {
    const result = buildCandidates(
      { generation: { hasResume: true, hasCoverLetter: false }, documents: [] },
      URL
    );
    expect(result[0]?.label).toBe('This job');
  });
});

// ---------------------------------------------------------------------------
// mountDocuments (the view)
// ---------------------------------------------------------------------------

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

const GENERATION_RESULT: PopupResponse = {
  ok: true,
  kind: 'documentsList',
  result: {
    ok: true,
    resource: 'documents',
    data: {
      generation: {
        hasResume: true,
        hasCoverLetter: true,
        jobTitle: 'Engineer',
        company: 'Acme',
      },
      documents: [],
    },
  },
  url: 'https://example.com/job/1',
};

describe('mountDocuments', () => {
  let host: HTMLElement;

  beforeEach(() => {
    document.body.innerHTML = '<div id="host"></div>';
    host = document.getElementById('host')!;
  });

  it('mounts showing nothing, and shows "Loading…" only while refresh() is genuinely in flight (#1225)', () => {
    const deps = makeDeps(vi.fn(() => new Promise<PopupResponse>(() => {})));
    const view = mountDocuments(host, deps);
    // The mount alone is NOT a fetch — no phantom "Loading…" (#1225).
    expect(host.textContent).not.toContain('Loading…');
    expect(host.textContent).toBe('');

    view.refresh();
    // Only now, with an unanswered request in flight, does the empty state
    // say "Loading…".
    expect(host.textContent).toContain('Loading…');
  });

  it('refresh() renders the résumé kind by default once a candidate resolves', async () => {
    const send = vi.fn(async () => GENERATION_RESULT);
    const deps = makeDeps(send);
    const view = mountDocuments(host, deps);
    view.refresh();
    await vi.waitFor(() => expect(host.querySelector('select')).not.toBeNull());

    expect(host.textContent).toContain('Attach résumé to this page');
    expect(deps.onUrlResolved).toHaveBeenCalledWith('https://example.com/job/1');
  });

  it('shows the empty state with a "Generate in the app" deep-link button that opens a tab on click', async () => {
    const send = vi.fn(
      async () =>
        ({
          ok: true,
          kind: 'documentsList',
          result: { ok: true, resource: 'documents', data: { generation: null, documents: [] } },
          url: 'https://example.com/job/1',
        }) satisfies PopupResponse
    );
    const view = mountDocuments(host, makeDeps(send));
    view.refresh();
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
    const send = vi.fn(
      async () =>
        ({
          ok: true,
          kind: 'documentsList',
          result: { ok: false, resource: 'documents', error: 'Assisted autofill is off.' },
          url: 'https://example.com/job/1',
        }) satisfies PopupResponse
    );
    const view = mountDocuments(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Assisted autofill is off.'));
    expect(host.querySelector('button.btn--quiet')).toBeNull();
  });

  it('disables the Cover letter toggle when the picked candidate has none', async () => {
    const send = vi.fn(
      async () =>
        ({
          ok: true,
          kind: 'documentsList',
          result: {
            ok: true,
            resource: 'documents',
            data: {
              generation: { hasResume: true, hasCoverLetter: false, jobTitle: 'Engineer' },
              documents: [],
            },
          },
          url: 'https://example.com/job/1',
        }) satisfies PopupResponse
    );
    const view = mountDocuments(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.querySelector('select')).not.toBeNull());

    const buttons = Array.from(host.querySelectorAll('button'));
    const letterBtn = buttons.find((b) => b.textContent === 'Cover letter');
    expect(letterBtn?.disabled).toBe(true);
  });

  it('Attach: asks confirmAttach first, then sends documentAttach and shows the result', async () => {
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind === 'documentsList') return GENERATION_RESULT;
      if (req.kind === 'documentAttach') {
        return {
          ok: true,
          kind: 'documentAttach',
          result: { attached: true, filename: 'resume.pdf', byteLength: 100 },
        } satisfies PopupResponse;
      }
      throw new Error(`unexpected request ${req.kind}`);
    });
    const deps = makeDeps(send);
    const view = mountDocuments(host, deps);
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Attach résumé to this page'));

    const attachBtn = Array.from(host.querySelectorAll('button')).find(
      (b) => b.textContent === 'Attach résumé to this page'
    )!;
    attachBtn.click();

    await vi.waitFor(() => expect(host.textContent).toContain('Attached resume.pdf'));
    expect(deps.confirmAttach).toHaveBeenCalledWith('example.com');
    expect(send).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'documentAttach', templateId: 'classic', format: 'pdf' })
    );
  });

  it('Attach: never sends the request when confirmAttach resolves false', async () => {
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind === 'documentsList') return GENERATION_RESULT;
      throw new Error(`unexpected request ${req.kind}`);
    });
    const deps = makeDeps(send);
    deps.confirmAttach = vi.fn(async () => false);
    const view = mountDocuments(host, deps);
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Attach résumé to this page'));

    const attachBtn = Array.from(host.querySelectorAll('button')).find(
      (b) => b.textContent === 'Attach résumé to this page'
    )!;
    attachBtn.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(send).not.toHaveBeenCalledWith(expect.objectContaining({ kind: 'documentAttach' }));
  });

  it('Copy cover letter: fetches the text and copies it', async () => {
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind === 'documentsList') return GENERATION_RESULT;
      if (req.kind === 'documentExportText') {
        return {
          ok: true,
          kind: 'documentExportText',
          text: 'Dear hiring manager…',
          filename: 'letter.txt',
        } satisfies PopupResponse;
      }
      throw new Error(`unexpected request ${req.kind}`);
    });
    const deps = makeDeps(send);
    const view = mountDocuments(host, deps);
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Attach résumé to this page'));

    // Switch to Cover letter.
    Array.from(host.querySelectorAll('button'))
      .find((b) => b.textContent === 'Cover letter')!
      .click();
    await vi.waitFor(() => expect(host.textContent).toContain('Copy cover letter'));

    Array.from(host.querySelectorAll('button'))
      .find((b) => b.textContent === 'Copy cover letter')!
      .click();

    await vi.waitFor(() => expect(deps.copy).toHaveBeenCalledWith('Dear hiring manager…'));
    await vi.waitFor(() => expect(host.textContent).toContain('Copied.'));
  });

  it('Paste cover letter: opens a field picker from the fed AnswerState and dispatches answerFill for an empty field', async () => {
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind === 'documentsList') return GENERATION_RESULT;
      if (req.kind === 'documentExportText') {
        return {
          ok: true,
          kind: 'documentExportText',
          text: 'Dear hiring manager…',
          filename: 'letter.txt',
        } satisfies PopupResponse;
      }
      if (req.kind === 'answerFill') {
        return {
          ok: true,
          kind: 'answerFill',
          result: { filled: true },
        } satisfies PopupResponse;
      }
      throw new Error(`unexpected request ${req.kind}`);
    });
    const view = mountDocuments(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Attach résumé to this page'));

    view.render({
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
    } satisfies AnswerState);

    Array.from(host.querySelectorAll('button'))
      .find((b) => b.textContent === 'Cover letter')!
      .click();
    await vi.waitFor(() => expect(host.textContent).toContain('Paste cover letter…'));

    Array.from(host.querySelectorAll('button'))
      .find((b) => b.textContent === 'Paste cover letter…')!
      .click();
    await vi.waitFor(() => expect(host.textContent).toContain('Cover letter'));

    const rowBtn = Array.from(host.querySelectorAll<HTMLButtonElement>('.picker__row'))[0]!;
    rowBtn.click();

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
    const send = vi.fn(async (req: PopupRequest) => {
      if (req.kind === 'documentsList') return GENERATION_RESULT;
      if (req.kind === 'documentExportText') {
        return new Promise<PopupResponse>((resolve) => {
          resolveExport = resolve;
        });
      }
      throw new Error(`unexpected request ${req.kind}`);
    });
    let followGeneration = 0;
    const view = mountDocuments(
      host,
      makeDeps(send, () => followGeneration)
    );
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Attach résumé to this page'));

    view.render({
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
    } satisfies AnswerState);

    Array.from(host.querySelectorAll('button'))
      .find((b) => b.textContent === 'Cover letter')!
      .click();
    await vi.waitFor(() => expect(host.textContent).toContain('Paste cover letter…'));

    Array.from(host.querySelectorAll('button'))
      .find((b) => b.textContent === 'Paste cover letter…')!
      .click();
    await vi.waitFor(() => expect(host.textContent).toContain('Cover letter'));

    const rowBtn = Array.from(host.querySelectorAll<HTMLButtonElement>('.picker__row'))[0]!;
    rowBtn.click();
    await vi.waitFor(() =>
      expect(send).toHaveBeenCalledWith(expect.objectContaining({ kind: 'documentExportText' }))
    );

    // The panel followed a different tab while the export above was still
    // in flight — the stale paste must never fire.
    followGeneration += 1;
    resolveExport?.({
      ok: true,
      kind: 'documentExportText',
      text: 'Dear hiring manager…',
      filename: 'letter.txt',
    });

    await vi.waitFor(() => expect(host.textContent).toContain('The followed tab changed'));
    expect(send).not.toHaveBeenCalledWith(expect.objectContaining({ kind: 'answerFill' }));
  });

  it('reset() clears candidates and any open picker, leaving no phantom "Loading…" (#1225)', async () => {
    const send = vi.fn(async () => GENERATION_RESULT);
    const view = mountDocuments(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Attach résumé to this page'));

    view.reset();
    expect(host.textContent).not.toContain('Loading…');
    expect(host.textContent).not.toContain('Attach résumé to this page');
  });

  it('reset(reason) renders the caller line and never a deep link (#1225)', async () => {
    const send = vi.fn(async () => GENERATION_RESULT);
    const view = mountDocuments(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() => expect(host.textContent).toContain('Attach résumé to this page'));

    // sidepanel.ts resets with the SHARED gated line on an untrusted tab.
    view.reset(JOB_TOOLS_GATED_LINE);
    expect(host.textContent).toContain(JOB_TOOLS_GATED_LINE);
    // A gated reset is untrusted — `lastUrl` is cleared, so the "Generate in
    // the app" deep link must never appear alongside the line.
    expect(host.querySelector('button.btn--quiet')).toBeNull();
  });

  it('kind-mismatch: surfaces an error instead of sitting on a phantom "Loading…" (#1225)', async () => {
    const send = vi.fn(
      async () =>
        ({ ok: true, kind: 'appliedCheck', result: { found: false } }) satisfies PopupResponse
    );
    const view = mountDocuments(host, makeDeps(send));
    view.refresh();
    await vi.waitFor(() =>
      expect(host.textContent).toContain('Unexpected response — please retry.')
    );
    expect(host.textContent).not.toContain('Loading…');
  });
});
