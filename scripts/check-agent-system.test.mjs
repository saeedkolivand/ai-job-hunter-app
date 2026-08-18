import { describe, expect, it } from 'vitest';

import { adrIndexProblems } from './check-agent-system.mjs';

// The ADR-index invariant only. `adrIndexProblems` takes the two things it
// compares as arguments, so every case here states its own on-disk set and its
// own README text — no fixture tree, and no dependence on which records happen
// to exist today.
//
// The two slugs below name REAL records, deliberately. This file is scanned by
// the sibling check-adr-citations guard like any other, and an open-series slug
// is a citation as far as that guard is concerned — `adr-013-…` reads as a
// citation of the open series' 13. So absence is expressed by leaving a record
// out of the on-disk array, never by inventing a number that resolves nowhere.

const CLOSED = '0013-email-confirmation-watching';
const OPEN = 'adr-013-resume-builder-base-plus-handoff';

const link = (slug) => `| [x](decision-records/${slug}.md) | a decision |`;

describe('adrIndexProblems', () => {
  it('accepts a record the README links', () => {
    expect(adrIndexProblems([CLOSED, OPEN], [link(CLOSED), link(OPEN)].join('\n'))).toEqual([]);
  });

  it('rejects a closed-series record named only in prose, with no index link', () => {
    // The hole this closes. A substring test passes on any mention, so the
    // table row can be deleted while a sentence elsewhere still names the
    // record — the README stops indexing it and nothing goes red. Only the
    // LINK counts, and the prose below deliberately contains the whole slug.
    const readme = [
      `Email confirmation watching is described in ${CLOSED} and remains in force.`,
      link(OPEN),
    ].join('\n');
    const problems = adrIndexProblems([CLOSED, OPEN], readme);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain(CLOSED);
    expect(problems[0]).toContain('missing index link');
  });

  it('rejects an open-series record named only in prose', () => {
    // Same hole, other series — the two are matched by one pattern, and a fix
    // that only covered the four-digit filenames would pass the case above.
    const readme = [`The base-plus-handoff split (${OPEN}) still holds.`, link(CLOSED)].join('\n');
    const problems = adrIndexProblems([CLOSED, OPEN], readme);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain(OPEN);
  });

  it('rejects a link whose record is not on disk', () => {
    // The other direction: the index outliving the file it points at.
    const problems = adrIndexProblems([OPEN], [link(CLOSED), link(OPEN)].join('\n'));
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain(`links ${CLOSED}`);
  });

  it('reports nothing when both sides are empty', () => {
    // Not a passing state worth trusting on its own, which is why the real
    // repo run is what the check actually gates on — recorded here so the
    // emptiness is a stated behaviour rather than an accident.
    expect(adrIndexProblems([], '')).toEqual([]);
  });
});
