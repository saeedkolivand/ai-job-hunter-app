/**
 * The Answer-tools component's COPY and its refusals.
 *
 * Everything asserted here is a claim the UI makes to the user, and every one
 * of them is a claim that is wrong in some state: an Accept sentence beside a
 * row with no field, a "grounded on" line under a rewrite that read nothing, a
 * "turn it on" hint matched against a string that is not actually the
 * desktop's refusal. Those are the cases pinned, not the happy path.
 */

import { describe, expect, it } from 'vitest';

import {
  EXTENSION_AI_ASSIST_OFF_MESSAGE,
  EXTENSION_NO_PROVIDER_MESSAGE,
} from '@ajh/shared/extension-protocol';

import {
  acceptSentence,
  fitLimitChip,
  gatedOffNotice,
  groundedOnLine,
  iterationHint,
  LENGTH_CHIPS,
  statusBadge,
  summaryLine,
  TONE_CHIPS,
} from './decisions';
import { row, stateOf } from './test-support';

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
    const rows = [row(), row({ id: 'b', status: 'filled' }), row({ id: 'c', status: 'drafted' })];
    expect(summaryLine(stateOf({ rows }))).toBe('3 questions · 1 to go');
  });
});
