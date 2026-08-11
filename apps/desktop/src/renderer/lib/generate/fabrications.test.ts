import { describe, expect, it } from 'vitest';

import { parseFabrications, presentFabrication, unresolvedCount } from './fabrications';

const VALID = {
  issueKey: 'factual.unsourced_metric#0',
  code: 'factual.unsourced_metric',
  evidence: 'Cut latency by 40%',
};

describe('parseFabrications', () => {
  it('accepts a well-formed list', () => {
    expect(parseFabrications([VALID])).toEqual([VALID]);
  });

  it('carries a valid decision through', () => {
    expect(parseFabrications([{ ...VALID, decision: 'keep' }])[0]?.decision).toBe('keep');
  });

  it('reads an unrecognized decision as UNDECIDED rather than as a verdict', () => {
    // Silently promoting garbage to "keep" would clear a review the user never did.
    expect(parseFabrications([{ ...VALID, decision: 'maybe' }])[0]?.decision).toBeUndefined();
  });

  it.each([
    ['not an array', 'nope'],
    ['absent', undefined],
    ['null', null],
    ['an object', { issueKey: 'x' }],
  ])('degrades %s to an empty list instead of throwing', (_label, input) => {
    expect(parseFabrications(input)).toEqual([]);
  });

  it.each([
    ['a missing issueKey', { code: 'c', evidence: 'e' }],
    ['a blank issueKey', { ...VALID, issueKey: '   ' }],
    ['a non-string code', { ...VALID, code: 7 }],
    ['a non-string evidence', { ...VALID, evidence: null }],
    ['a bare string', 'nonsense'],
  ])('drops %s rather than half-loading it', (_label, entry) => {
    expect(parseFabrications([entry, VALID])).toEqual([VALID]);
  });

  it('collapses duplicate issueKeys to the first — the key is the write identity', () => {
    const parsed = parseFabrications([VALID, { ...VALID, evidence: 'different text' }]);
    expect(parsed).toHaveLength(1);
    expect(parsed[0]?.evidence).toBe(VALID.evidence);
  });
});

describe('presentFabrication', () => {
  const document = 'Summary\nCut latency by 40% across the fleet.';

  it('is pending while undecided and the evidence is still there', () => {
    expect(presentFabrication(VALID, document)).toBe('pending');
  });

  it('is resolved once a verdict exists — even if the text has since gone', () => {
    expect(presentFabrication({ ...VALID, decision: 'remove' }, 'unrelated')).toBe('resolved');
  });

  // A preserved entry can outlive the line it describes: the user hand-edited
  // it away, or a "Re-check" carried the list across a newer document. Asking
  // someone to judge a bullet they cannot find is the failure this prevents;
  // the entry stays DECIDABLE because deciding it clears needs-review.
  it('is orphaned when the evidence no longer occurs in the document', () => {
    expect(presentFabrication(VALID, 'Summary\nLed the migration.')).toBe('orphaned');
  });

  it('treats empty evidence as orphaned, not as matching everything', () => {
    expect(presentFabrication({ ...VALID, evidence: '   ' }, document)).toBe('orphaned');
  });
});

describe('unresolvedCount', () => {
  it('counts only the undecided entries — what keeps a run needsReview', () => {
    expect(
      unresolvedCount([
        VALID,
        { ...VALID, issueKey: 'a', decision: 'keep' },
        { ...VALID, issueKey: 'b' },
      ])
    ).toBe(2);
  });
});
