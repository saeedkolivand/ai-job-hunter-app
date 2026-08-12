import { describe, expect, it } from 'vitest';

import {
  type Fabrication,
  isFabricationResolved,
  parseFabrications,
  presentFabrication,
  removeEvidenceLines,
  unresolvedCount,
} from './fabrications';

const VALID: Fabrication = {
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

  // `line` is what a removal is anchored to — see `removeEvidenceLines`.
  describe('the line anchor', () => {
    it('carries the containing line through', () => {
      const line = '- Cut latency by 40% across the fleet.';
      expect(parseFabrications([{ ...VALID, line }])[0]?.line).toBe(line);
    });

    // Absence is a first-class state: every report written before the field
    // existed has none. Dropping those entries would strand the run at
    // `needsReview` with nothing left to decide — far worse than losing the
    // automatic apply, which refuses honestly on its own.
    it.each([
      ['a legacy entry with no line', VALID],
      ['a non-string line', { ...VALID, line: 42 }],
    ])('keeps %s, reading the anchor as absent', (_label, entry) => {
      const parsed = parseFabrications([entry]);
      expect(parsed).toHaveLength(1);
      expect(parsed[0]?.line).toBeUndefined();
    });
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
  const BULLET = '- Cut latency by 40% across the fleet.';
  const document = ['Summary', BULLET, 'Shipped the rewrite.'].join('\n');
  /** The shape the report writes now: the span AND the line it sat on. */
  const anchored: Fabrication = { ...VALID, line: BULLET };

  it('deletes the whole line the entry was flagged on, not just the span', () => {
    // Excising the span alone would leave "- … across the fleet." dangling — a
    // worse document than either verdict.
    expect(removeEvidenceLines(document, anchored)).toBe('Summary\nShipped the rewrite.');
  });

  it('REFUSES when the anchor line is duplicated, rather than deleting both', () => {
    // The report only issues an anchor for a span that sat on exactly one
    // line, so a duplicate means the document moved after the report was
    // built. Résumés legitimately repeat a bullet across employers — deleting
    // every copy would silently edit more than the verdict covered.
    const twice = [BULLET, 'Summary', BULLET].join('\n');
    expect(removeEvidenceLines(twice, anchored)).toBeNull();
  });

  it('matches across leading/trailing whitespace drift', () => {
    // Re-indentation is not a different line. Anything beyond that is.
    expect(removeEvidenceLines(`Summary\n   ${BULLET}  \nShipped the rewrite.`, anchored)).toBe(
      'Summary\nShipped the rewrite.'
    );
  });

  it('drops the last line without leaving a trailing blank', () => {
    expect(removeEvidenceLines(`Summary\n${BULLET}`, anchored)).toBe('Summary');
  });

  // ── THE finding ───────────────────────────────────────────────────────────
  //
  // Validator evidence is routinely a bare token, and the old implementation
  // located the line by searching the document for it: every line CONTAINING
  // those characters was deleted. Executed against a real document, evidence
  // "250" took out the contact header (the phone number contains the digits),
  // "kubernetes" took out the whole skills line, and "a" took out half the file.
  //
  // Mutation check: relax the anchor back to `documentText.includes(evidence)`
  // (or match a line by `line.includes(anchor)` instead of equality) and the
  // first case below deletes the header and fails.
  describe('never deletes a line that merely CONTAINS the evidence', () => {
    const resume = [
      'ADA LOVELACE',
      '+1 (555) 250-8817 · ada@example.test',
      '',
      'EXPERIENCE',
      '- Cut cloud spend by 250k in one quarter.',
      '- Ran the kubernetes migration.',
      'Skills: kubernetes, terraform, go',
    ].join('\n');

    it('keeps the contact header when the evidence is a bare number it contains', () => {
      const entry: Fabrication = {
        issueKey: 'factual.unsourced_metric#0',
        code: 'factual.unsourced_metric',
        evidence: '250',
        line: '- Cut cloud spend by 250k in one quarter.',
      };
      const next = removeEvidenceLines(resume, entry);
      expect(next).toContain('+1 (555) 250-8817 · ada@example.test');
      expect(next).not.toContain('Cut cloud spend');
      // …and nothing else moved.
      expect(next?.split('\n')).toHaveLength(resume.split('\n').length - 1);
    });

    it('keeps the skills line when the evidence is a term it also lists', () => {
      const entry: Fabrication = {
        issueKey: 'factual.unsourced_term#1',
        code: 'factual.unsourced_term',
        evidence: 'kubernetes',
        line: '- Ran the kubernetes migration.',
      };
      const next = removeEvidenceLines(resume, entry);
      expect(next).toContain('Skills: kubernetes, terraform, go');
      expect(next).not.toContain('Ran the kubernetes migration');
    });
  });

  // Refusal is the honest outcome, and the caller already renders it: the
  // verdict is recorded and the row says "still in the document — edit it out".
  // Guessing a line from the span is the alternative, and it is the bug.
  it.each<[string, Fabrication]>([
    // A report persisted before `line` existed, or one a Re-check preserved.
    ['a legacy entry carrying no line at all', VALID],
    ['a blank line anchor', { ...VALID, line: '   ' }],
    [
      'a line the document no longer has (hand-edited since)',
      { ...VALID, line: '- Grew revenue 3x year over year.' },
    ],
    [
      'a line the user has since edited rather than deleted',
      { ...VALID, line: '- Cut latency by 40% across the whole fleet.' },
    ],
  ])('REFUSES (null) for %s rather than guessing from the evidence', (_label, entry) => {
    expect(removeEvidenceLines(document, entry)).toBeNull();
  });
});
