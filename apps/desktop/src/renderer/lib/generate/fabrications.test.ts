import { describe, expect, it } from 'vitest';

import {
  isFabricationResolved,
  parseFabrications,
  presentFabrication,
  removeEvidenceLines,
  unresolvedCount,
} from './fabrications';

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

  it('is resolved once Remove has actually taken the line out', () => {
    expect(presentFabrication({ ...VALID, decision: 'remove' }, 'unrelated')).toBe('resolved');
  });

  it('is resolved on Keep, whose whole point is that the text stays', () => {
    expect(presentFabrication({ ...VALID, decision: 'keep' }, document)).toBe('resolved');
  });

  // The finding this state exists for: "Remove" recorded, nothing removed. A
  // verdict is intent; the document is fact. Until they agree, the entry is not
  // finished and must not read as if it were.
  it('is markedForRemoval when Remove was recorded but the line is still there', () => {
    expect(presentFabrication({ ...VALID, decision: 'remove' }, document)).toBe('markedForRemoval');
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

describe('isFabricationResolved', () => {
  const document = 'Summary\nCut latency by 40% across the fleet.';

  it('is false while undecided', () => {
    expect(isFabricationResolved(VALID, document)).toBe(false);
  });

  it('is true for Keep — nothing has to change', () => {
    expect(isFabricationResolved({ ...VALID, decision: 'keep' }, document)).toBe(true);
  });

  it('is true for Remove ONLY once the evidence is gone', () => {
    const removed = { ...VALID, decision: 'remove' } as const;
    expect(isFabricationResolved(removed, document)).toBe(false);
    expect(isFabricationResolved(removed, 'Summary\nLed the migration.')).toBe(true);
  });
});

describe('unresolvedCount', () => {
  const document = 'Summary\nCut latency by 40% across the fleet.\nShipped the rewrite.';

  it('counts the undecided entries — what keeps a run needsReview', () => {
    expect(
      unresolvedCount(
        [VALID, { ...VALID, issueKey: 'a', decision: 'keep' }, { ...VALID, issueKey: 'b' }],
        document
      )
    ).toBe(2);
  });

  // Mutation guard: count any `decision` as resolved (drop the absence check)
  // and this is the assertion that fails — which is the whole point, because
  // that mutation is what lets the integrity chip go green over a line the user
  // is still looking at.
  it('does NOT count a recorded-but-unapplied Remove as resolved', () => {
    const marked = { ...VALID, decision: 'remove' } as const;
    expect(unresolvedCount([marked], document)).toBe(1);
    expect(unresolvedCount([marked], 'Summary\nShipped the rewrite.')).toBe(0);
  });
});

describe('removeEvidenceLines', () => {
  const document = ['Summary', 'Cut latency by 40% across the fleet.', 'Shipped the rewrite.'].join(
    '\n'
  );

  it('deletes the whole line the evidence sits on, not just the span', () => {
    // Excising the span alone would leave "across the fleet." dangling — a
    // worse document than either verdict.
    expect(removeEvidenceLines(document, 'Cut latency by 40%')).toBe(
      'Summary\nShipped the rewrite.'
    );
  });

  // The span rarely starts the line — a real bullet begins "- " or "• ". A cut
  // anchored on the SPAN (rather than the line) leaves the orphaned marker
  // glued to the next bullet; only a mid-line fixture can catch that.
  it('takes the bullet marker with it when the evidence starts mid-line', () => {
    const bulleted = ['Summary', '- Cut latency by 40% across the fleet.', '- Shipped it.'].join(
      '\n'
    );
    expect(removeEvidenceLines(bulleted, 'Cut latency by 40%')).toBe('Summary\n- Shipped it.');
  });

  it('removes EVERY occurrence, so the entry can actually reach resolved', () => {
    const twice = ['Cut latency by 40%.', 'Summary', 'Cut latency by 40% again.'].join('\n');
    const next = removeEvidenceLines(twice, 'Cut latency by 40%');
    expect(next).toBe('Summary');
    expect(next?.includes('Cut latency by 40%')).toBe(false);
  });

  it('spans multiple lines when the evidence does', () => {
    const wrapped = 'Summary\nCut latency\nby 40%\nShipped the rewrite.';
    expect(removeEvidenceLines(wrapped, 'Cut latency\nby 40%')).toBe(
      'Summary\nShipped the rewrite.'
    );
  });

  it('drops the last line without leaving a trailing blank', () => {
    expect(removeEvidenceLines('Summary\nCut latency by 40%', 'Cut latency by 40%')).toBe(
      'Summary'
    );
  });

  it.each([
    ['evidence that is not in the document', 'Grew revenue 3x'],
    ['blank evidence', '   '],
  ])('returns null for %s rather than mangling the text', (_label, evidence) => {
    expect(removeEvidenceLines(document, evidence)).toBeNull();
  });
});
