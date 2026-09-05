/**
 * Unit tests for the job-tools component: the pure response-decision helpers
 * (moved here from popup.test.ts unchanged in behavior) plus the module's own
 * new pieces — the trust gate ({@link isPageTrusted}) and the mounted
 * component's button/gating behavior, driven with a bare `<div>` host and a
 * mocked `send`, without going through either popup.ts or sidepanel.ts.
 */

import { describe, expect, it, vi } from 'vitest';

import type { AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import {
  IMPORT_LABEL_DEFAULT,
  isPageTrusted,
  JOB_TOOLS_GATED_LINE,
  type JobToolsDeps,
  mountJobTools,
  resolveAnswersSaveResponse,
  resolveFieldsProbeResponse,
  resolveFillResponse,
  resolveImportResponse,
  resolveMatchLiveResponse,
} from './job-tools';

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

function answerState(over: Partial<AnswerState> = {}): AnswerState {
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

// ── resolveImportResponse ─────────────────────────────────────────────────

describe('resolveImportResponse', () => {
  it('returns an error message when ok=false', () => {
    const res = { ok: false as const, error: 'Bridge unavailable.' };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('err');
    expect(text).toBe('Bridge unavailable.');
  });

  it('returns the unexpected-response error message when kind is not import', () => {
    const res = { ok: true as const, kind: 'token' as const };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('err');
    expect(text).toBe('Unexpected response — please retry.');
  });

  it('returns the result error text when the import result carries an error', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { error: 'Desktop app rejected the job URL.' },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('err');
    expect(text).toBe('Desktop app rejected the job URL.');
  });

  it('names the imported job and points to where it landed when a title is present', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-123', status: 'saved', title: 'Senior Rust Engineer' },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      'Imported “Senior Rust Engineer”. Open AI Job Hunter → Applications to view it.'
    );
  });

  it('falls back to a generic success + landing hint when no title is present', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-456' },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe('Imported. Open AI Job Hunter → Applications to view it.');
  });

  it('shows a partial message with title when partial=true', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-789', title: 'Frontend Engineer', partial: true },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      "Imported “Frontend Engineer” — couldn't read the description. Open AI Job Hunter → Applications to paste it."
    );
  });

  it('surfaces an "already tracked" transparency message when the matched row is already past saved and the checkbox was unticked', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-existing', status: 'applied', title: 'Backend Engineer' },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      '“Backend Engineer” is already tracked as Applied — status unchanged. Open AI Job Hunter → Applications to view it.'
    );
  });

  it('does not show the transparency message when the checkbox was ticked, even for a non-saved status', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-2', status: 'applied', title: 'DevOps Engineer' },
    };
    const { text, tone } = resolveImportResponse(res, true);
    expect(tone).toBe('ok');
    expect(text).toBe('Imported “DevOps Engineer”. Open AI Job Hunter → Applications to view it.');
  });

  it('appends the percent-fit suffix when matchScore is present', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: {
        applicationId: 'app-score',
        status: 'saved',
        title: 'Rust Engineer',
        matchScore: 71.6,
      },
    };
    const { text } = resolveImportResponse(res, false);
    expect(text).toBe(
      'Imported “Rust Engineer”. Open AI Job Hunter → Applications to view it. — 72% fit.'
    );
  });
});

// ── resolveFillResponse ─────────────────────────────────────────────────────

describe('resolveFillResponse', () => {
  it('surfaces the desktop refusal as an error', () => {
    const res = { ok: false as const, error: 'Autofill is off.' };
    const { text, tone } = resolveFillResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Autofill is off.');
  });

  it('reports the no-match case as a benign message, not an error', () => {
    const res = {
      ok: true as const,
      kind: 'fill' as const,
      summary: { filled: [], nameSplit: null, filledNothing: true },
    };
    const { text, tone } = resolveFillResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('No matchable fields found on this page.');
  });

  it('summarises the filled count and points the user at the page', () => {
    const res = {
      ok: true as const,
      kind: 'fill' as const,
      summary: {
        filled: [
          { key: 'email', label: 'Email', count: 2 },
          { key: 'phone', label: 'Phone', count: 1 },
        ],
        nameSplit: null,
        filledNothing: false,
      },
    };
    const { text, tone } = resolveFillResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('Filled 3 fields — review them on the page.');
  });

  it('flags the name-split guess in the confirmation', () => {
    const res = {
      ok: true as const,
      kind: 'fill' as const,
      summary: {
        filled: [{ key: 'firstName', label: 'First name', count: 1 }],
        nameSplit: { first: 'Saeed', last: 'Kolivand' },
        filledNothing: false,
      },
    };
    const { text } = resolveFillResponse(res);
    expect(text).toBe('Filled 1 field — review them on the page (name split is a guess — verify).');
  });
});

// ── resolveFieldsProbeResponse ──────────────────────────────────────────────

describe('resolveFieldsProbeResponse', () => {
  it('returns both signals on success', () => {
    const res = {
      ok: true as const,
      kind: 'fieldsProbe' as const,
      hasFormFields: true,
      hasAnswerFields: false,
    };
    expect(resolveFieldsProbeResponse(res)).toEqual({
      showFormGroup: true,
      showAnswerTools: false,
    });
  });

  it('fails OPEN (both true) on a transport-level ok:false', () => {
    const res = { ok: false as const, error: 'message channel closed' };
    expect(resolveFieldsProbeResponse(res)).toEqual({ showFormGroup: true, showAnswerTools: true });
  });

  it('fails OPEN (both true) for an unexpected response kind', () => {
    const res = { ok: true as const, kind: 'token' as const };
    expect(resolveFieldsProbeResponse(res)).toEqual({ showFormGroup: true, showAnswerTools: true });
  });
});

// ── resolveAnswersSaveResponse ───────────────────────────────────────────────

describe('resolveAnswersSaveResponse', () => {
  it('names the job with title @ company and the saved count on success', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: {
        ok: true as const,
        applicationId: 'app-1',
        saved: 7,
        skipped: 2,
        title: 'Backend Engineer',
        company: 'Acme',
      },
    };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('Saved 7 answers to Backend Engineer @ Acme — 2 already recorded.');
  });

  it('falls back to a generic "no new answers" message when saved and skipped are both 0', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: { ok: true as const, applicationId: 'app-1', saved: 0, skipped: 0 },
    };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('No new answers to save from this page.');
  });

  it('shows a distinct "already recorded" message when saved is 0 but skipped is not', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: { ok: true as const, applicationId: 'app-1', saved: 0, skipped: 3 },
    };
    const { text } = resolveAnswersSaveResponse(res);
    expect(text).toBe('All 3 answers were already recorded.');
  });

  it('surfaces the desktop refusal text when result.ok is false', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: { ok: false as const, error: "couldn't find a saved job for this page" },
    };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe("couldn't find a saved job for this page");
  });
});

// ── resolveMatchLiveResponse ─────────────────────────────────────────────────

describe('resolveMatchLiveResponse', () => {
  it('surfaces a transport-level error with null score fields', () => {
    const res = { ok: false as const, error: 'Desktop app not reachable.' };
    const view = resolveMatchLiveResponse(res);
    expect(view.tone).toBe('err');
    expect(view.score).toBeNull();
    expect(view.gaps).toEqual([]);
  });

  it('surfaces the desktop refusal text when result.ok is false', () => {
    const res = {
      ok: true as const,
      kind: 'matchLive' as const,
      result: {
        ok: false as const,
        error: 'Add a resume in AI Job Hunter first, then try Check fit again.',
      },
    };
    const view = resolveMatchLiveResponse(res);
    expect(view.tone).toBe('err');
    expect(view.score).toBeNull();
  });

  it('renders the rounded score, source label, résumé name, and gaps on success', () => {
    const res = {
      ok: true as const,
      kind: 'matchLive' as const,
      result: {
        ok: true as const,
        combined: 71.6,
        ats: 60,
        gaps: ['kubernetes', 'terraform'],
        resumeName: 'My Resume',
        scoreSource: 'keyword' as const,
      },
    };
    const view = resolveMatchLiveResponse(res);
    expect(view.tone).toBe('ok');
    expect(view.score).toBe(72);
    expect(view.scoreLabel).toBe('keyword coverage');
    expect(view.resumeName).toBe('My Resume');
    expect(view.gaps).toEqual(['kubernetes', 'terraform']);
    expect(view.text).toBe('72% fit against “My Resume”.');
  });
});

// ── isPageTrusted (the trust gate) ───────────────────────────────────────────

describe('isPageTrusted', () => {
  it('is untrusted when no record exists for the tab', () => {
    expect(isPageTrusted(null)).toBe(false);
  });

  it('is untrusted for a record whose page has changed', () => {
    expect(isPageTrusted(answerState({ pageChanged: true }))).toBe(false);
  });

  it('is trusted only for an existing record with pageChanged:false', () => {
    expect(isPageTrusted(answerState({ pageChanged: false }))).toBe(true);
  });
});

// ── mountJobTools: the four actions ──────────────────────────────────────────

function mount(sendImpl?: (req: PopupRequest) => Promise<PopupResponse>) {
  const host = document.createElement('div');
  const send = vi.fn(
    sendImpl ?? (async (): Promise<PopupResponse> => ({ ok: false, error: 'not configured' }))
  );
  const onAnswerToolsVisibility = vi.fn();
  const deps: JobToolsDeps = { send, onAnswerToolsVisibility };
  const view = mountJobTools(host, deps);
  return { host, send, onAnswerToolsVisibility, view };
}

const msg = (host: HTMLElement) => host.querySelector<HTMLParagraphElement>('#job-tools-msg')!;

describe('doImport (#btn-import)', () => {
  it('shows "Importing…" then the success message, sends applied:false by default, and re-enables the button', async () => {
    const { host, send } = mount(async () => ({
      ok: true,
      kind: 'import',
      result: { applicationId: 'app-1', status: 'saved', title: 'Rust Engineer' },
    }));
    const btn = host.querySelector<HTMLButtonElement>('#btn-import')!;

    btn.click();
    expect(btn.disabled).toBe(true);
    expect(msg(host).textContent).toBe('Importing…');

    await flush();

    expect(msg(host).textContent).toBe(
      'Imported “Rust Engineer”. Open AI Job Hunter → Applications to view it.'
    );
    expect(btn.disabled).toBe(false);
    expect(send).toHaveBeenCalledWith({ kind: 'import', applied: false });
  });

  it('sends applied:true when the "I already applied" checkbox is ticked', async () => {
    const { host, send } = mount(async () => ({
      ok: true,
      kind: 'import',
      result: { applicationId: 'app-1' },
    }));
    host.querySelector<HTMLInputElement>('#chk-applied')!.checked = true;
    host.querySelector<HTMLButtonElement>('#btn-import')!.click();
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'import', applied: true });
  });

  it('shows a retry message and re-enables the button on a transport rejection', async () => {
    const { host } = mount(async () => {
      throw new Error('message channel closed');
    });
    const btn = host.querySelector<HTMLButtonElement>('#btn-import')!;
    btn.click();
    await flush();

    expect(msg(host).textContent).toBe('Import failed. Please retry.');
    expect(btn.disabled).toBe(false);
  });
});

describe('doFill (#btn-fill)', () => {
  it('shows "Filling…" then the success summary, and re-enables the button', async () => {
    const { host, send } = mount(async () => ({
      ok: true,
      kind: 'fill',
      summary: {
        filled: [{ key: 'email', label: 'Email', count: 1 }],
        nameSplit: null,
        filledNothing: false,
      },
    }));
    const btn = host.querySelector<HTMLButtonElement>('#btn-fill')!;

    btn.click();
    expect(btn.disabled).toBe(true);
    expect(msg(host).textContent).toBe('Filling…');

    await flush();

    expect(msg(host).textContent).toBe('Filled 1 field — review them on the page.');
    expect(btn.disabled).toBe(false);
    expect(send).toHaveBeenCalledWith({ kind: 'fill' });
  });

  it('shows a retry message on rejection', async () => {
    const { host } = mount(async () => {
      throw new Error('boom');
    });
    host.querySelector<HTMLButtonElement>('#btn-fill')!.click();
    await flush();

    expect(msg(host).textContent).toBe('Autofill failed. Please retry.');
  });
});

describe('doCheckFit (#btn-check-fit)', () => {
  it('renders the score card and re-enables the button on success', async () => {
    const { host, send } = mount(async () => ({
      ok: true,
      kind: 'matchLive',
      result: {
        ok: true,
        combined: 72,
        ats: 60,
        gaps: ['kubernetes', 'terraform'],
        resumeName: 'My Resume',
        scoreSource: 'keyword',
      },
    }));
    const btn = host.querySelector<HTMLButtonElement>('#btn-check-fit')!;
    btn.click();
    await flush();

    const card = host.querySelector<HTMLDivElement>('#match-result')!;
    expect(card.hidden).toBe(false);
    expect(card.textContent).toContain('72% fit');
    expect(card.textContent).toContain('kubernetes');
    expect(msg(host).textContent).toBe('72% fit against “My Resume”.');
    expect(btn.disabled).toBe(false);
    expect(send).toHaveBeenCalledWith({ kind: 'matchLive' });
  });

  it('hides the score card and surfaces the desktop refusal (no résumé saved yet)', async () => {
    const { host } = mount(async () => ({
      ok: true,
      kind: 'matchLive',
      result: {
        ok: false,
        error: 'Add a resume in AI Job Hunter first, then try Check fit again.',
      },
    }));
    host.querySelector<HTMLButtonElement>('#btn-check-fit')!.click();
    await flush();

    expect(host.querySelector<HTMLDivElement>('#match-result')!.hidden).toBe(true);
    expect(msg(host).textContent).toBe(
      'Add a resume in AI Job Hunter first, then try Check fit again.'
    );
  });
});

describe('doSaveAnswers (#btn-save-answers)', () => {
  it('shows "Saving your answers…" then the success confirmation', async () => {
    const { host, send } = mount(async () => ({
      ok: true,
      kind: 'answersSave',
      result: {
        ok: true,
        applicationId: 'app-1',
        saved: 7,
        skipped: 0,
        title: 'Backend Engineer',
        company: 'Acme',
      },
      filled: [],
    }));
    const btn = host.querySelector<HTMLButtonElement>('#btn-save-answers')!;

    btn.click();
    expect(btn.disabled).toBe(true);
    expect(msg(host).textContent).toBe('Saving your answers…');

    await flush();

    expect(msg(host).textContent).toBe('Saved 7 answers to Backend Engineer @ Acme.');
    expect(btn.disabled).toBe(false);
    expect(send).toHaveBeenCalledWith({ kind: 'answersSave' });
  });

  it('surfaces the desktop refusal text (errors ARE shown)', async () => {
    const { host } = mount(async () => ({
      ok: true,
      kind: 'answersSave',
      result: { ok: false, error: "couldn't find a saved job for this page — import it first" },
      filled: [],
    }));
    host.querySelector<HTMLButtonElement>('#btn-save-answers')!.click();
    await flush();

    expect(msg(host).textContent).toBe("couldn't find a saved job for this page — import it first");
  });
});

// ── the fields probe (Form group visibility) ─────────────────────────────────

describe('checkPage — fields probe (Form group + onAnswerToolsVisibility)', () => {
  it('hides the Form group and forwards showAnswerTools:false when the probe finds no fields', async () => {
    const { host, send, onAnswerToolsVisibility, view } = mount(async () => ({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: false,
      hasAnswerFields: false,
    }));
    view.checkPage();
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'fieldsProbe' });
    expect(host.querySelector<HTMLElement>('#group-form')!.hidden).toBe(true);
    expect(onAnswerToolsVisibility).toHaveBeenCalledWith(false);
  });

  it('fails OPEN on a transport rejection', async () => {
    const { host, onAnswerToolsVisibility, view } = mount(async () => {
      throw new Error('message channel closed');
    });
    view.checkPage();
    await flush();

    expect(host.querySelector<HTMLElement>('#group-form')!.hidden).toBe(false);
    expect(onAnswerToolsVisibility).toHaveBeenCalledWith(true);
  });

  it('a stale response from an earlier checkPage() call must not clobber a newer one (generation guard)', async () => {
    let resolveFirst: ((res: PopupResponse) => void) | undefined;
    const first = new Promise<PopupResponse>((resolve) => {
      resolveFirst = resolve;
    });
    const send = vi
      .fn<(req: PopupRequest) => Promise<PopupResponse>>()
      .mockReturnValueOnce(first)
      .mockResolvedValueOnce({
        ok: true,
        kind: 'fieldsProbe',
        hasFormFields: true,
        hasAnswerFields: true,
      });
    const host = document.createElement('div');
    const view = mountJobTools(host, { send });

    view.checkPage(); // first call — left pending
    view.checkPage(); // second call — resolves first, "wins"
    await flush();

    expect(host.querySelector<HTMLElement>('#group-form')!.hidden).toBe(false);

    // The first call's stale "no fields" response arrives late — must be a no-op.
    resolveFirst?.({ ok: true, kind: 'fieldsProbe', hasFormFields: false, hasAnswerFields: false });
    await flush();

    expect(host.querySelector<HTMLElement>('#group-form')!.hidden).toBe(false);
  });
});

// ── the trust gate's effect on rendering ─────────────────────────────────────

describe('render — the trust gate', () => {
  it('starts trusted (active controls shown, gated line hidden) before any state is known', () => {
    const { host } = mount();
    expect(host.querySelector<HTMLElement>('#job-tools-gated')!.hidden).toBe(true);
    expect(host.querySelector<HTMLElement>('#job-tools-active')!.hidden).toBe(false);
  });

  it('replaces the four controls with the gated line for a navigated (pageChanged) tab', () => {
    const { host, view } = mount();
    view.render(answerState({ pageChanged: true }));

    expect(host.querySelector<HTMLElement>('#job-tools-gated')!.hidden).toBe(false);
    expect(host.querySelector<HTMLElement>('#job-tools-gated')!.textContent).toBe(
      JOB_TOOLS_GATED_LINE
    );
    expect(host.querySelector<HTMLElement>('#job-tools-active')!.hidden).toBe(true);
  });

  it('replaces the four controls with the SAME gated line when no record exists at all', () => {
    const { host, view } = mount();
    view.render(null);

    expect(host.querySelector<HTMLElement>('#job-tools-gated')!.hidden).toBe(false);
  });

  it('unblocks the controls again once a fresh, untraveled record arrives (a valid gesture landed)', () => {
    const { host, view } = mount();
    view.render(answerState({ pageChanged: true }));
    expect(host.querySelector<HTMLElement>('#job-tools-active')!.hidden).toBe(true);

    view.render(answerState({ pageChanged: false }));

    expect(host.querySelector<HTMLElement>('#job-tools-gated')!.hidden).toBe(true);
    expect(host.querySelector<HTMLElement>('#job-tools-active')!.hidden).toBe(false);
  });

  it('does not call send for checkPage while untrusted', async () => {
    const { send, view } = mount();
    view.render(answerState({ pageChanged: true }));
    view.checkPage();
    await flush();

    expect(send).not.toHaveBeenCalled();
  });

  it('re-runs the fields probe automatically when regaining trust while mounted (the panel never remounts on a toolbar-click re-grant)', async () => {
    const { send, view } = mount(async () => ({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: true,
      hasAnswerFields: true,
    }));
    view.render(answerState({ pageChanged: true })); // untrusted
    view.render(answerState({ pageChanged: false })); // regained trust
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'fieldsProbe' });
  });
});

// ── setImportLabel / reset (the popup's own appliedCheck seam) ───────────────

describe('setImportLabel and reset', () => {
  it('setImportLabel overrides the Import button text', () => {
    const { host, view } = mount();
    view.setImportLabel('Re-import / update');
    expect(host.querySelector<HTMLButtonElement>('#btn-import')!.textContent).toBe(
      'Re-import / update'
    );
  });

  it('reset restores the default Import label, hides the match card, and re-shows the Form group', () => {
    const { host, onAnswerToolsVisibility, view } = mount();
    view.setImportLabel('Re-import / update');
    view.reset();

    expect(host.querySelector<HTMLButtonElement>('#btn-import')!.textContent).toBe(
      IMPORT_LABEL_DEFAULT
    );
    expect(host.querySelector<HTMLElement>('#match-result')!.hidden).toBe(true);
    expect(host.querySelector<HTMLElement>('#group-form')!.hidden).toBe(false);
    expect(onAnswerToolsVisibility).toHaveBeenCalledWith(true);
  });
});

// ── per-instance isolation ────────────────────────────────────────────────────

describe('two mounted instances (popup + panel) never share state', () => {
  it('a generation bump in one instance does not affect the other', async () => {
    let resolveA: ((res: PopupResponse) => void) | undefined;
    const pendingA = new Promise<PopupResponse>((resolve) => {
      resolveA = resolve;
    });
    const sendA = vi
      .fn<(req: PopupRequest) => Promise<PopupResponse>>()
      .mockReturnValueOnce(pendingA);
    const sendB = vi.fn<(req: PopupRequest) => Promise<PopupResponse>>().mockResolvedValueOnce({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: false,
      hasAnswerFields: false,
    });
    const hostA = document.createElement('div');
    const hostB = document.createElement('div');
    const viewA = mountJobTools(hostA, { send: sendA });
    const viewB = mountJobTools(hostB, { send: sendB });

    viewA.checkPage();
    viewB.checkPage(); // a DIFFERENT instance's generation counter
    await flush();

    // B's own probe resolved and hid its OWN Form group.
    expect(hostB.querySelector<HTMLElement>('#group-form')!.hidden).toBe(true);
    // A's is still pending, so A's Form group is untouched (still visible).
    expect(hostA.querySelector<HTMLElement>('#group-form')!.hidden).toBe(false);

    resolveA?.({ ok: true, kind: 'fieldsProbe', hasFormFields: false, hasAnswerFields: false });
    await flush();

    expect(hostA.querySelector<HTMLElement>('#group-form')!.hidden).toBe(true);
    // B's own generation was never bumped by A's calls.
    expect(sendB).toHaveBeenCalledTimes(1);
  });
});
