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
import {
  acceptSentence,
  fitLimitChip,
  gatedOffNotice,
  groundedOnLine,
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
});
