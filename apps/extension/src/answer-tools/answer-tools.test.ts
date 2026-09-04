/**
 * The Answer-tools component's COPY and its refusals.
 *
 * Everything asserted here is a claim the UI makes to the user, and every one
 * of them is a claim that is wrong in some state: an Accept sentence beside a
 * row with no field, a "grounded on" line under a rewrite that read nothing, a
 * "turn it on" hint matched against a string that is not actually the
 * desktop's refusal. Those are the cases pinned, not the happy path.
 */

import { describe, expect, it, vi } from 'vitest';

import {
  EXTENSION_AI_ASSIST_OFF_MESSAGE,
  EXTENSION_NO_PROVIDER_MESSAGE,
} from '@ajh/shared/extension-protocol';

import type { AnswerRow, AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import {
  acceptSentence,
  fitLimitChip,
  gatedOffNotice,
  groundedOnLine,
  iterationHint,
  LENGTH_CHIPS,
  mountAnswerTools,
  statusBadge,
  summaryLine,
  TONE_CHIPS,
} from './answer-tools';

const row = (over: Partial<AnswerRow> = {}): AnswerRow => ({
  id: 'r',
  question: 'Why do you want to work here?',
  field: { kind: 'empty', index: 0, count: 1, currentText: '', originalText: '' },
  status: 'empty',
  versions: [],
  selected: -1,
  ...over,
});

describe('gatedOffNotice', () => {
  it('recognises both shared refusal sentinels and says what still works', () => {
    for (const sentinel of [EXTENSION_AI_ASSIST_OFF_MESSAGE, EXTENSION_NO_PROVIDER_MESSAGE]) {
      const notice = gatedOffNotice(sentinel);
      expect(notice).toContain(sentinel);
      expect(notice).toMatch(/keep working while drafting is off/);
    }
  });

  it('does not dress up an error that is NOT one of the sentinels', () => {
    // Every other `ok:false` error is opaque to the client and is rendered
    // verbatim. Adding "turn it on in Settings" to, say, a transport failure
    // would send the user to a setting that is already on.
    expect(gatedOffNotice('Could not reach the desktop app.')).toBeNull();
    expect(gatedOffNotice(undefined)).toBeNull();
    // A near-miss must not match either — the sentinel IS the code.
    expect(gatedOffNotice(`${EXTENSION_AI_ASSIST_OFF_MESSAGE} `)).toBeNull();
  });
});

describe('acceptSentence', () => {
  it('names the exact question it would overwrite', () => {
    const sentence = acceptSentence(
      row({ versions: [{ label: 'v1', text: 'A draft.', kind: 'draft' }], selected: 0 }),
      false
    );

    expect(sentence).toContain('Why do you want to work here?');
    expect(sentence).toContain('Nothing else is touched');
  });

  it('is absent when there is no Accept to explain', () => {
    const drafted = {
      versions: [{ label: 'v1', text: 'A draft.', kind: 'draft' as const }],
      selected: 0,
    };
    // No field on the page…
    expect(acceptSentence(row({ ...drafted, field: null }), false)).toBeNull();
    // …and after a navigation, where the write control is replaced entirely.
    expect(acceptSentence(row(drafted), true)).toBeNull();
  });
});

describe('groundedOnLine', () => {
  it('always names the résumé for a draft, and adds only the flags the wire set', () => {
    const line = groundedOnLine(
      row({
        selected: 0,
        versions: [
          {
            label: 'v1',
            text: 'x',
            kind: 'draft',
            sourced: { web: false, brief: true, salary: false },
          },
        ],
      })
    );

    expect(line).toContain('your résumé');
    expect(line).toContain('this posting');
    expect(line).not.toContain('web search');
  });

  it('claims nothing for a rewrite, which read neither the résumé nor the posting', () => {
    expect(
      groundedOnLine(row({ selected: 0, versions: [{ label: 'v1', text: 'x', kind: 'rewrite' }] }))
    ).toBeNull();
  });

  it('claims nothing for the page’s own text', () => {
    expect(groundedOnLine(row({ selected: -1 }))).toBeNull();
  });
});

describe('iterationHint', () => {
  it('names the posting only when the selected draft actually used one', () => {
    const groundedOnPosting = iterationHint(
      row({
        selected: 0,
        versions: [{ label: 'v1', text: 'x', kind: 'draft', sourced: { brief: true } }],
      })
    );
    expect(groundedOnPosting).toContain('and this posting');

    // Drafted before a job was matched — `groundedOnLine` shows no posting
    // for this same version, so the hint must not claim one either.
    const notGroundedOnPosting = iterationHint(
      row({
        selected: 0,
        versions: [{ label: 'v1', text: 'x', kind: 'draft', sourced: { brief: false } }],
      })
    );
    expect(notGroundedOnPosting).not.toContain('and this posting');
  });

  it('never claims the posting for a rewrite, which read neither', () => {
    expect(
      iterationHint(row({ selected: 0, versions: [{ label: 'v1', text: 'x', kind: 'rewrite' }] }))
    ).not.toContain('and this posting');
  });
});

describe('the chip rows', () => {
  it('each start with an explicit neutral that sends nothing', () => {
    for (const chips of [TONE_CHIPS, LENGTH_CHIPS]) {
      const first = chips[0]!;
      expect(first.label).toBe('As is');
      expect(first.preset).toBeUndefined();
      expect(first.instruction).toBeUndefined();
    }
  });

  it('carry either a preset or a free instruction, never both', () => {
    for (const chip of [...TONE_CHIPS, ...LENGTH_CHIPS].filter((c) => c.label !== 'As is')) {
      expect(Boolean(chip.preset) !== Boolean(chip.instruction), chip.label).toBe(true);
    }
  });
});

describe('fitLimitChip', () => {
  const capped = row({
    field: { kind: 'empty', index: 0, count: 1, currentText: '', originalText: '', maxChars: 10 },
  });

  it('appears only when the text is actually over, and carries the MEASURED overshoot', () => {
    const chip = fitLimitChip(capped, 'x'.repeat(14));
    expect(chip?.label).toBe('Fit 10');
    expect(chip?.instruction).toContain('14 characters');
    expect(chip?.instruction).toContain('Cut at least 4');
  });

  it('is absent at the limit, and absent when the field declares none', () => {
    expect(fitLimitChip(capped, 'x'.repeat(10))).toBeNull();
    expect(fitLimitChip(row(), 'x'.repeat(10_000))).toBeNull();
  });
});

describe('statusBadge / summaryLine', () => {
  it('names the latest version on a drafted row', () => {
    expect(
      statusBadge(
        row({
          status: 'drafted',
          selected: 0,
          versions: [
            { label: 'v1', text: 'a', kind: 'draft' },
            { label: 'v2', text: 'b', kind: 'rewrite' },
          ],
        })
      )
    ).toBe('v2 ready');
  });

  it('says a free-text row is not on the page, so nobody expects an Accept', () => {
    expect(statusBadge(row({ field: null }))).toBe('Not on page');
  });

  it('counts what is left to do, and says so before anything is scanned', () => {
    expect(summaryLine(null)).toBe('Nothing scanned yet');
    const state: AnswerState = {
      tabId: 1,
      origin: 'https://example.com',
      scannedAt: 0,
      stream: null,
      pageChanged: false,
      rows: [row(), row({ id: 'b', status: 'filled' }), row({ id: 'c', status: 'drafted' })],
    };
    expect(summaryLine(state)).toBe('3 questions · 1 to go');
  });
});

describe('mountAnswerTools (the rendered row)', () => {
  const state = (over: Partial<AnswerState> = {}): AnswerState => ({
    tabId: 1,
    origin: 'https://boards.example.com',
    scannedAt: 0,
    stream: null,
    pageChanged: false,
    rows: [
      row({
        status: 'drafted',
        selected: 0,
        versions: [{ label: 'v1', text: 'A drafted answer.', kind: 'draft' }],
        field: {
          kind: 'empty',
          index: 0,
          count: 1,
          currentText: '',
          originalText: '',
          maxChars: 10,
        },
      }),
    ],
    ...over,
  });

  const mount = () => {
    const host = document.createElement('div');
    document.body.append(host);
    const send = vi.fn(async () => ({
      ok: true as const,
      kind: 'answerState' as const,
      state: null,
    }));
    const view = mountAnswerTools(host, { send, copy: vi.fn(async () => true) });
    return { host, view, send };
  };

  const openFirstRow = (host: HTMLElement) => {
    host.querySelector<HTMLButtonElement>('.arow__head')?.click();
  };

  it('renders the composer for the open row without touching the page', () => {
    const { host, view } = mount();
    view.render(state());
    openFirstRow(host);

    expect(host.textContent).toContain('A drafted answer.');
    // The counter measures the text on screen against the field's own limit.
    expect(host.textContent).toContain('17 / 10 characters');
    // Copy is the primary action; Accept is the quiet one beside it.
    expect(host.querySelector('.btn--primary')?.textContent).toBe('Copy');
    expect(host.textContent).toContain('Accept into field');
    expect(host.textContent).toContain('Nothing else is touched');
    // Over the limit, so the fit-the-limit chip is offered — and only then.
    expect(host.textContent).toContain('Fit 10');
  });

  it('replaces every write control with one line after a navigation, keeping the rows', () => {
    const { host, view } = mount();
    view.render(state({ pageChanged: true }));
    openFirstRow(host);

    // The row and its draft are still there — decision 3 keeps them.
    expect(host.textContent).toContain('A drafted answer.');
    expect(host.textContent).toContain('Click the toolbar icon');
    // …and nothing that would read or write the page is offered.
    expect(host.textContent).not.toContain('Accept into field');
    expect(host.querySelector('.chip')).toBeNull();
  });

  it('offers no Accept for a question that is not on the page', () => {
    const { host, view } = mount();
    view.render(
      state({
        rows: [
          row({
            field: null,
            status: 'drafted',
            selected: 0,
            versions: [{ label: 'v1', text: 'Copy-only.', kind: 'draft' }],
          }),
        ],
      })
    );
    openFirstRow(host);

    expect(host.textContent).toContain('Copy-only.');
    expect(host.textContent).not.toContain('Accept into field');
    expect(host.textContent).not.toContain('Nothing else is touched');
  });

  it('shows the gated-off sentinel with where to turn it on, and what still works', () => {
    const { host, view } = mount();
    view.render(state({ rows: [row({ error: EXTENSION_AI_ASSIST_OFF_MESSAGE })] }));
    openFirstRow(host);

    expect(host.textContent).toContain(EXTENSION_AI_ASSIST_OFF_MESSAGE);
    expect(host.textContent).toContain('keep working while drafting is off');
  });

  it('sends a rewrite over the existing wire verb when a chip is pressed', () => {
    const { host, view, send } = mount();
    view.render(state());
    openFirstRow(host);

    const shorter = [...host.querySelectorAll<HTMLButtonElement>('.chip')].find(
      (b) => b.textContent === 'Shorter'
    );
    shorter?.click();

    expect(send).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'answerAssist', mode: 'rewrite', preset: 'shorten' })
    );
  });

  it('does not send anything for the explicit "As is" neutral', () => {
    const { host, view, send } = mount();
    view.render(state());
    openFirstRow(host);

    [...host.querySelectorAll<HTMLButtonElement>('.chip')]
      .filter((b) => b.textContent === 'As is')
      .forEach((b) => b.click());

    expect(send).not.toHaveBeenCalled();
  });

  it('renders the "Left as it is." notice immediately on click — the ONLY feedback this no-op control gives (regression)', () => {
    const { host, view } = mount();
    view.render(state());
    openFirstRow(host);

    const asIs = [...host.querySelectorAll<HTMLButtonElement>('.chip')].find(
      (b) => b.textContent === 'As is'
    );
    asIs?.click();

    expect(host.querySelector('.msg')?.textContent).toBe('Left as it is.');
  });

  // ── render() tears the whole section down on every call (a stream tick
  // anywhere in the tab, via storage.onChanged) — Findings 1/2 ────────────────

  it('keeps the "add a question" input\'s typed text AND focus/caret across a re-render', () => {
    const { host, view } = mount();
    view.render(state());

    const addInput = host.querySelector<HTMLInputElement>('[data-focus-key="add-question"]');
    if (!addInput) throw new Error('expected the add-question input');
    addInput.value = 'What is your salary expectation?';
    addInput.dispatchEvent(new Event('input'));
    addInput.focus();
    addInput.setSelectionRange(4, 8);

    // Simulates a `storage.onChanged` push for a stream progressing on ANY
    // row in the tab — `render()` rebuilds unconditionally on every call.
    view.render(state());

    const restored = host.querySelector<HTMLInputElement>('[data-focus-key="add-question"]');
    expect(restored).not.toBeNull();
    // Mutation guard: without the value backing, this is '' — without focus
    // restoration, `document.activeElement` is `document.body`.
    expect(restored?.value).toBe('What is your salary expectation?');
    expect(restored).toBe(document.activeElement);
    expect(restored?.selectionStart).toBe(4);
    expect(restored?.selectionEnd).toBe(8);
  });

  it("keeps a row's instruction input FOCUSED (with its caret) across a re-render", () => {
    const { host, view } = mount();
    view.render(state());
    openFirstRow(host);

    const instruction = host.querySelector<HTMLInputElement>('.arow__instruction');
    if (!instruction) throw new Error('expected the row instruction input');
    instruction.value = 'Mention Berlin';
    instruction.dispatchEvent(new Event('input'));
    instruction.focus();
    instruction.setSelectionRange(2, 2);

    view.render(state());

    const restored = host.querySelector<HTMLInputElement>('.arow__instruction');
    // Mutation guard: without focus restoration this is `document.body`.
    expect(restored).toBe(document.activeElement);
    expect(restored?.selectionStart).toBe(2);
  });

  it('keeps the Rescan button FOCUSED across a re-render', () => {
    const { host, view } = mount();
    view.render(state());

    const rescan = host.querySelector<HTMLButtonElement>('[data-focus-key="rescan"]');
    if (!rescan) throw new Error('expected the Rescan button');
    rescan.focus();

    // Simulates a `storage.onChanged` push for a stream progressing on ANY
    // row in the tab — `render()` rebuilds unconditionally on every call.
    view.render(state());

    const restored = host.querySelector<HTMLButtonElement>('[data-focus-key="rescan"]');
    // Mutation guard: without a `data-focus-key` on this button, focus lands
    // on `document.body` after every render — several times a second while
    // anything streams.
    expect(restored).not.toBeNull();
    expect(restored).toBe(document.activeElement);
  });

  it("keeps a row's head toggle FOCUSED across a re-render", () => {
    const { host, view } = mount();
    view.render(state());
    openFirstRow(host);

    const head = host.querySelector<HTMLButtonElement>('.arow__head');
    if (!head) throw new Error('expected the row head toggle');
    head.focus();

    view.render(state());

    const restored = host.querySelector<HTMLButtonElement>('.arow__head');
    // Mutation guard: without a `data-focus-key` on this button, focus lands
    // on `document.body` after every render.
    expect(restored).not.toBeNull();
    expect(restored).toBe(document.activeElement);
  });

  // ── Rescan vs `pageChanged` and a RESOLVED (not thrown) failure — Finding 3 ─

  it('disables Rescan once the page has changed, mirroring the per-row write controls', () => {
    const { host, view } = mount();
    view.render(state({ pageChanged: true }));

    const rescan = [...host.querySelectorAll<HTMLButtonElement>('.atools__head button')].find(
      (b) => b.textContent === 'Rescan'
    );
    expect(rescan?.disabled).toBe(true);
  });

  it('surfaces a RESOLVED answerScan failure (not just a thrown one) as an error notice', async () => {
    const host = document.createElement('div');
    document.body.append(host);
    const send = vi.fn(async (req: PopupRequest): Promise<PopupResponse> =>
      req.kind === 'answerScan'
        ? { ok: false, error: 'Could not read the questions on this page.' }
        : { ok: true, kind: 'answerState', state: null }
    );
    const view = mountAnswerTools(host, { send, copy: vi.fn(async () => true) });
    view.render(state({ rows: [] }));

    const rescan = [...host.querySelectorAll<HTMLButtonElement>('.atools__head button')].find(
      (b) => b.textContent === 'Rescan'
    );
    rescan?.click();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(host.querySelector('.msg--err')?.textContent).toBe(
      'Could not read the questions on this page.'
    );
  });

  // ── The SHARED stream gates controls, not just this view's own `busy` —
  // Finding 5 (a stream another surface started, or this view reattaching
  // mid-stream, leaves `busy` false here) ─────────────────────────────────────

  it('disables chips/Accept/Regenerate while the shared stream targets this row, even with busy=false', () => {
    const { host, view } = mount();
    view.render(
      state({
        stream: {
          rowId: 'r',
          text: 'partial answer',
          done: false,
          interrupted: false,
          kind: 'rewrite',
        },
      })
    );
    openFirstRow(host);

    const nonNeutralChips = [...host.querySelectorAll<HTMLButtonElement>('.chip')].filter(
      (b) => !b.classList.contains('chip--neutral')
    );
    expect(nonNeutralChips.length).toBeGreaterThan(0);
    // Mutation guard: without gating on `streaming`, every one of these is
    // enabled here (`busy` starts `false` on a fresh mount).
    expect(nonNeutralChips.every((b) => b.disabled)).toBe(true);

    const buttons = [...host.querySelectorAll<HTMLButtonElement>('.btn')];
    expect(buttons.find((b) => b.textContent === 'Accept into field')?.disabled).toBe(true);
    expect(buttons.find((b) => b.textContent === 'Regenerate')?.disabled).toBe(true);
  });

  it('does NOT disable controls for a stream on a DIFFERENT row', () => {
    const { host, view } = mount();
    view.render(
      state({
        stream: {
          rowId: 'some-other-row',
          text: 'x',
          done: false,
          interrupted: false,
          kind: 'rewrite',
        },
      })
    );
    openFirstRow(host);

    const buttons = [...host.querySelectorAll<HTMLButtonElement>('.btn')];
    expect(buttons.find((b) => b.textContent === 'Accept into field')?.disabled).toBe(false);
  });
});
