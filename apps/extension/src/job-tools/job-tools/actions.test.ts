/**
 * The mounted job-tools component's four actions — Import, Fill (+ its confirm
 * step and copy-field fallback), Check fit, Save my answers — plus the results
 * stamp button, driven with a bare `<div>` host and a mocked `send`.
 */

import { describe, expect, it, vi } from 'vitest';

import { setStampResultsPages } from '../../lib/appearance';
import type { PopupRequest, PopupResponse } from '../../lib/messages';
import { answerState, btn, FILLED_NOTHING, fillRouter, flush, mount, msg } from '../test-support';

/** A `fill` reply that matched one email field. */
const FILLED_EMAIL: PopupResponse = {
  ok: true,
  kind: 'fill',
  summary: {
    filled: [{ key: 'email', label: 'Email', count: 1 }],
    nameSplit: null,
    filledNothing: false,
  },
};
const profile = (result: Record<string, unknown>): PopupResponse =>
  ({ ok: true, kind: 'profileGet', result }) as PopupResponse;
const rejecting = (message: string) => async (): Promise<never> => {
  throw new Error(message);
};

describe('doImport (#btn-import)', () => {
  it('shows "Importing…" then the success message, sends applied:false by default, and re-enables the button', async () => {
    const { host, send } = mount(async () => ({
      ok: true,
      kind: 'import',
      result: { applicationId: 'app-1', status: 'saved', title: 'Rust Engineer' },
    }));
    const importBtn = btn(host, '#btn-import');

    importBtn.click();
    expect(importBtn.disabled).toBe(true);
    expect(msg(host).textContent).toBe('Importing…');

    await flush();

    expect(msg(host).textContent).toBe(
      'Imported “Rust Engineer”. Open AI Job Hunter → Applications to view it.'
    );
    expect(importBtn.disabled).toBe(false);
    expect(send).toHaveBeenCalledWith({ kind: 'import', applied: false });
  });

  it('sends applied:true when the "I already applied" checkbox is ticked', async () => {
    const { host, send } = mount(async () => ({
      ok: true,
      kind: 'import',
      result: { applicationId: 'app-1' },
    }));
    host.querySelector<HTMLInputElement>('#chk-applied')!.checked = true;
    btn(host, '#btn-import').click();
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'import', applied: true });
  });

  it('shows a retry message and re-enables the button on a transport rejection', async () => {
    const { host } = mount(rejecting('message channel closed'));
    const importBtn = btn(host, '#btn-import');
    importBtn.click();
    await flush();

    expect(msg(host).textContent).toBe('Import failed. Please retry.');
    expect(importBtn.disabled).toBe(false);
  });
});

describe('doFill (#btn-fill)', () => {
  it('shows "Filling…" then the success summary, and re-enables the button', async () => {
    const { host, send } = mount(async () => FILLED_EMAIL);
    const fillBtn = btn(host, '#btn-fill');

    fillBtn.click();
    expect(fillBtn.disabled).toBe(true);
    expect(msg(host).textContent).toBe('Filling…');

    await flush();

    expect(msg(host).textContent).toBe('Filled 1 field — review them on the page.');
    expect(fillBtn.disabled).toBe(false);
    expect(send).toHaveBeenCalledWith({ kind: 'fill' });
  });

  it('shows a retry message on rejection', async () => {
    const { host } = mount(rejecting('boom'));
    btn(host, '#btn-fill').click();
    await flush();

    expect(msg(host).textContent).toBe('Autofill failed. Please retry.');
  });

  it('asks confirmFill first and skips the request when it resolves false', async () => {
    const confirmFill = vi.fn(async () => false);
    const { host, send } = mount(undefined, { confirmFill });

    btn(host, '#btn-fill').click();
    await flush();

    expect(confirmFill).toHaveBeenCalledTimes(1);
    expect(send).not.toHaveBeenCalled();
    expect(msg(host).textContent).toBe('');
  });

  it('proceeds with the fill request when confirmFill resolves true', async () => {
    const confirmFill = vi.fn(async () => true);
    const { host, send } = mount(async () => FILLED_NOTHING, { confirmFill });

    btn(host, '#btn-fill').click();
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'fill' });
  });

  it('locks the button before awaiting confirmFill, so a repeated click cannot start a second confirmation or double-send fill', async () => {
    let resolveConfirm: ((v: boolean) => void) | undefined;
    const confirmFill = vi.fn(
      () =>
        new Promise<boolean>((resolve) => {
          resolveConfirm = resolve;
        })
    );
    const { host, send } = mount(async () => FILLED_NOTHING, { confirmFill });
    const fillBtn = btn(host, '#btn-fill');

    fillBtn.click();
    expect(fillBtn.disabled).toBe(true);

    // Repeated clicks while the first confirmation is still pending.
    fillBtn.click();
    fillBtn.click();

    resolveConfirm?.(true);
    await flush();

    expect(confirmFill).toHaveBeenCalledTimes(1);
    // The mocked `fill` response comes back `filledNothing: true`, which also
    // triggers ONE `profileGet` fetch for the copy-field fallback (decision
    // 8) — assert the `fill` request itself was never double-sent, rather
    // than the raw call count.
    expect(send.mock.calls.filter(([req]) => req.kind === 'fill')).toHaveLength(1);
    expect(fillBtn.disabled).toBe(false);
  });
});

describe('copy-field fallback (decision 8)', () => {
  const fallback = (host: HTMLElement) =>
    host.querySelector<HTMLElement>('#job-tools-profile-fallback')!;

  /** Mount with a fill that matched nothing, click Fill and let it settle. */
  async function filled(profileReply?: PopupResponse, extra: Parameters<typeof mount>[1] = {}) {
    const mounted = mount(fillRouter(FILLED_NOTHING, profileReply), extra);
    btn(mounted.host, '#btn-fill').click();
    await flush();
    return mounted;
  }

  it('fetches the profile and renders Copy-able fields when Fill comes back filledNothing', async () => {
    const { host, send } = await filled(
      profile({ fullName: 'Ada Lovelace', email: 'ada@example.com' })
    );

    expect(send).toHaveBeenCalledWith({ kind: 'profileGet' });
    const section = fallback(host);
    expect(section.hidden).toBe(false);
    expect(section.textContent).toContain('Ada Lovelace');
    expect(section.textContent).toContain('ada@example.com');
    expect(section.querySelectorAll('button')).toHaveLength(2);
  });

  it('renders nothing when the profile fetch refuses (Autofill opt-in off)', async () => {
    const { host } = await filled(profile({ error: 'Not paired.' }));

    expect(fallback(host).hidden).toBe(true);
  });

  it('never fetches the profile, and hides any previously shown fallback, when Fill matched something', async () => {
    const { host, send } = mount(fillRouter(FILLED_EMAIL));

    btn(host, '#btn-fill').click();
    await flush();

    expect(send).not.toHaveBeenCalledWith({ kind: 'profileGet' });
    expect(fallback(host).hidden).toBe(true);
  });

  it("copies a field's value via deps.copy when its Copy button is clicked", async () => {
    const copy = vi.fn(async () => true);
    const { host } = await filled(profile({ email: 'ada@example.com' }), { copy });

    btn(fallback(host), 'button').click();
    await flush();

    expect(copy).toHaveBeenCalledWith('ada@example.com');
  });

  it('discards a stale profileGet reply when render() switches tabs while it is in flight', async () => {
    let resolveProfile: ((res: PopupResponse) => void) | undefined;
    const { host, send, view } = mount(async (req: PopupRequest) => {
      if (req.kind === 'profileGet') {
        return new Promise<PopupResponse>((resolve) => {
          resolveProfile = resolve;
        });
      }
      return FILLED_NOTHING;
    });

    btn(host, '#btn-fill').click();
    await flush();
    expect(send).toHaveBeenCalledWith({ kind: 'profileGet' });

    // The panel followed a different tab while the profileGet fetch above
    // was still in flight — this must invalidate it.
    view.render(answerState({ tabId: 2 }));

    resolveProfile?.(profile({ fullName: 'Ada Lovelace', email: 'ada@example.com' }));
    await flush();

    expect(fallback(host).hidden).toBe(true);
    expect(fallback(host).textContent).not.toContain('Ada Lovelace');
  });

  it('reset() hides an open fallback', async () => {
    const { host, view } = await filled(profile({ email: 'ada@example.com' }));
    expect(fallback(host).hidden).toBe(false);

    view.reset();
    expect(fallback(host).hidden).toBe(true);
  });
});

describe('hideSaveAnswers (popup three-action rule)', () => {
  it('hides "Save my answers from this page" when set', () => {
    const { host } = mount(undefined, { hideSaveAnswers: true });

    expect(btn(host, '#btn-save-answers').hidden).toBe(true);
  });

  it('shows it by default (the side panel keeps all four controls)', () => {
    const { host } = mount();
    expect(btn(host, '#btn-save-answers').hidden).toBe(false);
  });
});

describe('doCheckFit (#btn-check-fit)', () => {
  const matched = (over: Record<string, unknown> = {}) =>
    ({
      ok: true,
      kind: 'matchLive',
      result: {
        ok: true,
        combined: 72,
        ats: 60,
        gaps: [],
        resumeName: 'My Resume',
        scoreSource: 'keyword',
        ...over,
      },
    }) as PopupResponse;
  const card = (host: HTMLElement) => host.querySelector<HTMLDivElement>('#match-result')!;

  it('renders the score card and re-enables the button on success', async () => {
    const { host, send } = mount(async () => matched({ gaps: ['kubernetes', 'terraform'] }));
    const checkBtn = btn(host, '#btn-check-fit');
    checkBtn.click();
    await flush();

    expect(card(host).hidden).toBe(false);
    expect(card(host).textContent).toContain('72% fit');
    expect(card(host).textContent).toContain('kubernetes');
    expect(msg(host).textContent).toBe('72% fit against “My Resume”.');
    expect(checkBtn.disabled).toBe(false);
    expect(send).toHaveBeenCalledWith({ kind: 'matchLive' });
  });

  it('hides the score card and surfaces the desktop refusal (no résumé saved yet)', async () => {
    const error = 'Add a resume in AI Job Hunter first, then try Check fit again.';
    const { host } = mount(async () => ({
      ok: true,
      kind: 'matchLive',
      result: { ok: false, error },
    }));
    btn(host, '#btn-check-fit').click();
    await flush();

    expect(card(host).hidden).toBe(true);
    expect(msg(host).textContent).toBe(error);
  });

  it('shows the two salary facts, verbatim and never a verdict, inside the why? details (PR3)', async () => {
    const { host } = mount(async () =>
      matched({ salary: { posting: '€70,000–€90,000', expectation: '€80,000' } })
    );
    btn(host, '#btn-check-fit').click();
    await flush();

    expect(card(host).textContent).toContain('Posting says €70,000–€90,000');
    expect(card(host).textContent).toContain('You want €80,000');
  });
});

describe('#btn-stamp-results visibility + doStampResults (PR3)', () => {
  it('stays hidden by default (results-stamp preference OFF)', async () => {
    await setStampResultsPages(false);
    const { host } = mount();
    await flush();
    expect(btn(host, '#btn-stamp-results').hidden).toBe(true);
  });

  it('shows once the results-stamp preference is ON, read live at mount (not cached)', async () => {
    await setStampResultsPages(true);
    const { host } = mount();
    await flush();
    expect(btn(host, '#btn-stamp-results').hidden).toBe(false);
    await setStampResultsPages(false); // reset for later tests in this file
  });

  it('sends stampResults and renders the returned status line', async () => {
    await setStampResultsPages(true);
    const { host, send } = mount(async () => ({
      ok: true,
      kind: 'stampResults',
      stamped: 2,
      status: 'Stamped 2 cards.',
    }));
    await flush();
    const stampBtn = btn(host, '#btn-stamp-results');
    expect(stampBtn.hidden).toBe(false);

    stampBtn.click();
    expect(msg(host).textContent).toBe('Stamping…');
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'stampResults' });
    expect(msg(host).textContent).toBe('Stamped 2 cards.');
    expect(stampBtn.disabled).toBe(false);
    await setStampResultsPages(false); // reset for later tests in this file
  });
});

describe('doSaveAnswers (#btn-save-answers)', () => {
  const saved = (result: Record<string, unknown>) =>
    ({ ok: true, kind: 'answersSave', result, filled: [] }) as PopupResponse;

  it('shows "Saving your answers…" then the success confirmation', async () => {
    const { host, send } = mount(async () =>
      saved({
        ok: true,
        applicationId: 'app-1',
        saved: 7,
        skipped: 0,
        title: 'Backend Engineer',
        company: 'Acme',
      })
    );
    const saveBtn = btn(host, '#btn-save-answers');

    saveBtn.click();
    expect(saveBtn.disabled).toBe(true);
    expect(msg(host).textContent).toBe('Saving your answers…');

    await flush();

    expect(msg(host).textContent).toBe('Saved 7 answers to Backend Engineer @ Acme.');
    expect(saveBtn.disabled).toBe(false);
    expect(send).toHaveBeenCalledWith({ kind: 'answersSave' });
  });

  it('surfaces the desktop refusal text (errors ARE shown)', async () => {
    const error = "couldn't find a saved job for this page — import it first";
    const { host } = mount(async () => saved({ ok: false, error }));
    btn(host, '#btn-save-answers').click();
    await flush();

    expect(msg(host).textContent).toBe(error);
  });
});
