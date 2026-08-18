import { describe, expect, it } from 'vitest';

import {
  adrFilenames,
  classifyAdrFiles,
  MIN_ADR_FILES,
  MIN_CITATIONS,
  MIN_HYPHEN_CITATIONS,
  MIN_PATH_REFS,
  MIN_SCANNED_FILES,
  MIN_SPACED_CITATIONS,
  readFiles,
  scan,
  scanText,
  trackedFiles,
  violations,
} from './check-adr-citations.mjs';

// Two kinds of test here, and the split matters.
//
// The functions are exercised against a synthetic ADR inventory and synthetic
// file text, so a case can state exactly what it is testing without depending
// on which ADRs happen to exist today. Then a handful of cases run against the
// REAL repo, because a guard whose discovery silently stops matching the actual
// source is the one failure mode that makes all of this theatre — and only the
// real repo can catch that.
//
// This file is excluded from the check's own scan (see EXCLUDED in the script):
// the fixtures below deliberately contain broken citations, and scanning them
// would make the guard fail on the tests that prove it works.

/** A synthetic inventory with both series populated, mirroring the real shape. */
const INV = classifyAdrFiles([
  '0013-email-confirmation-watching.md',
  '0020-crash-reporting.md',
  'adr-013-resume-builder-base-plus-handoff.md',
  'adr-027-diagnostics-bundle-privacy-boundary.md',
  'README-not-an-adr.md',
]);

/** Stats that clear every vacuity floor, so a case can vary one thing at a time. */
const healthy = (over = {}) => ({
  pathRefs: MIN_PATH_REFS + 1,
  citations: MIN_CITATIONS + 1,
  byForm: { hyphen: MIN_HYPHEN_CITATIONS + 1, spaced: MIN_SPACED_CITATIONS + 1 },
  badPaths: [],
  badCites: [],
  ambiguous: [],
  ...over,
});

/** An inventory big enough to clear the ADR floor. */
const bigInv = classifyAdrFiles(
  Array.from({ length: MIN_ADR_FILES + 1 }, (_, i) => `adr-${String(i + 1).padStart(3, '0')}-x.md`)
);

describe('classifyAdrFiles', () => {
  it('splits the two series by filename shape', () => {
    expect([...INV.closed.keys()]).toEqual(['0013', '0020']);
    expect([...INV.open.keys()]).toEqual(['013', '027']);
  });

  it('keys the two series so the SAME number can live in both', () => {
    // The whole reason this check exists: 13 is email-watching in one series
    // and the resume builder in the other, and they must not collide.
    expect(INV.closed.get('0013')).toBe('0013-email-confirmation-watching.md');
    expect(INV.open.get('013')).toBe('adr-013-resume-builder-base-plus-handoff.md');
  });

  it('reports a file matching neither series instead of ignoring it', () => {
    // An ADR nothing can cite by number is a real problem, not a nuisance.
    expect(INV.unclassified).toEqual(['README-not-an-adr.md']);
  });

  it('reports two files claiming one number instead of letting one overwrite the other', () => {
    // A map keyed by identifier holds one file per key, so the second write
    // silently wins and every citation of 013 then names two records while
    // still "resolving". Retaining the claim is what makes that visible.
    const dup = classifyAdrFiles([
      'adr-013-resume-builder-base-plus-handoff.md',
      'adr-013-something-else.md',
    ]);
    expect(dup.duplicates).toHaveLength(1);
    expect(dup.duplicates[0]).toContain('adr-013-resume-builder-base-plus-handoff.md');
    expect(dup.duplicates[0]).toContain('adr-013-something-else.md');
    // Both files stay resolvable as PATHS — they are both on disk.
    expect(dup.all).toHaveLength(2);
  });

  it('resolves a path to the file a duplicate number displaced from the map', () => {
    // The displaced file exists; reporting its path as dangling would be a
    // false positive pointing at a file the reader can open.
    const dup = classifyAdrFiles([
      'adr-013-resume-builder-base-plus-handoff.md',
      'adr-013-something-else.md',
    ]);
    const r = scanText('a.md', 'see decision-records/adr-013-something-else.md', dup);
    expect(r.pathRefs).toBe(1);
    expect(r.badPaths).toEqual([]);
  });

  it('finds no duplicate when each number is claimed once', () => {
    // Guards the reverse: a duplicate detector that always fires is no better
    // than one that never does.
    expect(INV.duplicates).toEqual([]);
  });
});

describe('scanText — number citations', () => {
  it('resolves a correctly padded citation in each series', () => {
    const r = scanText('a.md', 'See ADR-0013 and ADR-013.', INV);
    expect(r.citations).toBe(2);
    expect(r.badCites).toEqual([]);
    expect(r.ambiguous).toEqual([]);
  });

  it('catches the real mis-padding defect: ADR 0027 in the closed series', () => {
    // The exact shipped defect. ADR-0020 cited the diagnostics-bundle ADR as
    // "ADR 0027"; adr-027 exists but 0027 does not, so four digits resolve to
    // nothing. Padding is the only signal, so this must not pass.
    const r = scanText('docs/x.md', 'a redacted diagnostics zip (ADR 0027)', INV);
    expect(r.badCites).toHaveLength(1);
    expect(r.badCites[0]).toContain('ADR 0027');
    expect(r.badCites[0]).toContain('closed series');
  });

  it('catches a number absent from both series', () => {
    const r = scanText('docs/x.md', 'per ADR-0025 and ADR-999', INV);
    expect(r.badCites).toHaveLength(2);
  });

  it('catches a 1- or 2-digit citation as ambiguous, not as resolved', () => {
    // "ADR-13" could mean either record. Silently picking one would be worse
    // than failing, so it is reported rather than resolved.
    const r = scanText('docs/x.md', 'ADR-13 and ADR-7 say so', INV);
    expect(r.ambiguous).toHaveLength(2);
    expect(r.badCites).toEqual([]);
    expect(r.ambiguous[0]).toContain('ADR-0013');
    expect(r.ambiguous[0]).toContain('ADR-013');
  });

  it('catches an over-padded number rather than failing to match it', () => {
    // `\d+` with a length check, not `\d{3,4}`: a pattern that simply does not
    // match a malformed citation reports zero problems and looks green.
    const r = scanText('docs/x.md', 'ADR-00013 is not a thing', INV);
    expect(r.citations).toBe(1);
    expect(r.badCites[0]).toContain('matches neither series');
  });

  it('matches the space form and lowercase, which the repo uses in both', () => {
    const r = scanText('docs/x.md', 'ADR 0013, adr-013, Adr 013', INV);
    expect(r.citations).toBe(3);
    expect(r.badCites).toEqual([]);
  });

  it('counts each form separately from the SAME pass', () => {
    // The separator is captured, not just matched, so the per-form floors and
    // the resolver can never be looking at different populations.
    const r = scanText('docs/x.md', 'ADR-0013 ADR 0013 adr-013 ADR 013', INV);
    expect(r.byForm).toEqual({ hyphen: 2, spaced: 2 });
    expect(r.byForm.hyphen + r.byForm.spaced).toBe(r.citations);
  });

  it('reports the line number so the citation can be found', () => {
    const r = scanText('docs/x.md', 'line one\nline two\nsee ADR-0099\n', INV);
    expect(r.badCites[0]).toContain('docs/x.md:3');
  });

  it('does not match a bare number that is not an ADR citation', () => {
    const r = scanText('docs/x.md', 'PR #1000 fixed 0013 issues in adr work', INV);
    expect(r.citations).toBe(0);
  });
});

describe('scanText — path references', () => {
  it('resolves a real path and counts it', () => {
    const r = scanText(
      'a.md',
      '[x](decision-records/adr-027-diagnostics-bundle-privacy-boundary.md)',
      INV
    );
    expect(r.pathRefs).toBe(1);
    expect(r.badPaths).toEqual([]);
  });

  it('catches a path whose file is gone', () => {
    const r = scanText('a.md', 'see docs/knowledge/decision-records/adr-999-ghost.md', INV);
    expect(r.badPaths).toHaveLength(1);
    expect(r.badPaths[0]).toContain('adr-999-ghost.md');
  });

  it('ignores a prose elision, which describes a filename rather than linking one', () => {
    // AUDIT_REPORT.md writes "decision-records/adr-007-...md" mid-sentence.
    const r = scanText('a.md', 'decision-records/adr-007-...md still says Accepted', INV);
    expect(r.pathRefs).toBe(0);
    expect(r.badPaths).toEqual([]);
  });
});

describe('scan', () => {
  it('aggregates across files and skips those that never mention an ADR', () => {
    const files = [
      { path: 'a.md', text: 'ADR-0013' },
      { path: 'b.md', text: 'nothing relevant here at all' },
      { path: 'c.md', text: 'ADR-0099' },
    ];
    const stats = scan(files, INV);
    expect(stats.citations).toBe(2);
    expect(stats.badCites).toHaveLength(1);
  });

  it('validates a closed-series path whose label never spells the three letters', () => {
    // The pre-filter's blind spot. A closed-series filename is all digits and
    // a slug, and `decision-records/` says nothing either, so a file linking
    // one under a plain-English label contains no `adr` substring at all — and
    // was skipped before the path was ever resolved. The link below is dead;
    // if the pre-filter drops the file, this reports zero problems and passes.
    const files = [{ path: 'd.md', text: '[Email watching](decision-records/0099-gone.md)' }];
    expect(/adr/i.test(files[0].text)).toBe(false); // the premise, asserted
    const stats = scan(files, INV);
    expect(stats.pathRefs).toBe(1);
    expect(stats.badPaths).toHaveLength(1);
    expect(stats.badPaths[0]).toContain('0099-gone.md');
  });
});

describe('violations — vacuity guards', () => {
  // Every check in this script is "the problem list is empty", which is exactly
  // the shape that passes when discovery breaks. Each case below supplies a
  // CLEAN problem set, so the only thing that can make it fail is the vacuity
  // guard itself.
  it('fails when the file walk yields almost nothing', () => {
    const problems = violations(healthy(), bigInv, MIN_SCANNED_FILES - 1);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('files scanned');
  });

  it('fails when the ADR inventory is nearly empty', () => {
    const problems = violations(healthy(), INV, MIN_SCANNED_FILES);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('ADRs found');
  });

  it('fails when the citation pattern stops matching entirely', () => {
    const dead = { citations: 0, byForm: { hyphen: 0, spaced: 0 } };
    const problems = violations(healthy(dead), bigInv, MIN_SCANNED_FILES);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('hyphen form');
    expect(problems[0]).toContain('spaced form');
    expect(problems[0]).toContain('backstop');
  });

  // The two that matter most. A form dying is the realistic failure — the
  // pattern stops matching `ADR NNN` while `ADR-NNN` keeps working — and a
  // total-only floor cannot see it, because the surviving form dilutes it.
  // Measured on the real repo: the spaced form is 143 of 913, so losing ALL
  // of it is a ~16% dip that any safe total-only floor sails past.
  it('fails when ONLY the spaced form dies, even though the total looks healthy', () => {
    const stats = healthy({ byForm: { hyphen: MIN_HYPHEN_CITATIONS + 200, spaced: 0 } });
    const problems = violations(stats, bigInv, MIN_SCANNED_FILES);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('spaced form');
    // The hyphen form survived, so its floor must NOT be implicated.
    expect(problems[0]).not.toContain('hyphen form');
  });

  it('fails when ONLY the hyphen form dies', () => {
    const stats = healthy({ byForm: { hyphen: 0, spaced: MIN_SPACED_CITATIONS + 50 } });
    const problems = violations(stats, bigInv, MIN_SCANNED_FILES);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('hyphen form');
    expect(problems[0]).not.toContain('spaced form');
  });

  it('fails when the path pattern stops matching', () => {
    const problems = violations(healthy({ pathRefs: 0 }), bigInv, MIN_SCANNED_FILES);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('decision-records path references');
  });

  it('reports the vacuity failure INSTEAD of a false all-clear', () => {
    // The failure this guards: zero files, zero citations, zero problems — a
    // green run that checked nothing. It must be red.
    const empty = { pathRefs: 0, citations: 0, badPaths: [], badCites: [], ambiguous: [] };
    expect(violations(empty, classifyAdrFiles([]), 0).length).toBeGreaterThan(0);
  });

  it('passes a healthy scan with no problems', () => {
    expect(violations(healthy(), bigInv, MIN_SCANNED_FILES)).toEqual([]);
  });
});

describe('violations — reporting', () => {
  it('reports each problem class once the vacuity floors are cleared', () => {
    const stats = healthy({
      badPaths: ['a.md:1 → decision-records/adr-999-ghost.md'],
      badCites: ['b.md:2 → ADR 0027'],
      ambiguous: ['c.md:3 → ADR-13'],
    });
    const problems = violations(stats, bigInv, MIN_SCANNED_FILES);
    expect(problems).toHaveLength(3);
    expect(problems.join('\n')).toContain('adr-999-ghost.md');
    expect(problems.join('\n')).toContain('ADR 0027');
    expect(problems.join('\n')).toContain('ADR-13');
  });

  it('reports two files claiming the same number', () => {
    // Without this the citation still resolves, so nothing else in the check
    // can notice — the ambiguity is in the inventory, not in any citation.
    const withDup = classifyAdrFiles([...adrFilenames(), 'adr-013-a-second-claim.md']);
    const problems = violations(healthy(), withDup, MIN_SCANNED_FILES);
    expect(problems.join('\n')).toContain('adr-013-a-second-claim.md');
    expect(problems.join('\n')).toContain('more than one file');
  });

  it('reports an ADR file belonging to neither series', () => {
    const withStray = classifyAdrFiles([...adrFilenames(), 'stray-note.md']);
    const problems = violations(healthy(), withStray, MIN_SCANNED_FILES);
    expect(problems.join('\n')).toContain('stray-note.md');
  });
});

// ── Against the REAL repo ───────────────────────────────────────────────────
// Everything above runs on fixtures, so all of it stays green if discovery
// drifts off the actual source. These do not.

describe('the real repo', () => {
  const inv = classifyAdrFiles(adrFilenames());
  const files = readFiles(trackedFiles());
  const stats = scan(files, inv);

  it('discovers both series, well clear of the vacuity floors', () => {
    expect(inv.closed.size).toBeGreaterThan(0);
    expect(inv.open.size).toBeGreaterThan(0);
    expect(inv.closed.size + inv.open.size).toBeGreaterThanOrEqual(MIN_ADR_FILES);
  });

  it('has no ADR file that belongs to neither series', () => {
    expect(inv.unclassified).toEqual([]);
  });

  it('has no number claimed by two records', () => {
    expect(inv.duplicates).toEqual([]);
    // Anchored to an absolute rather than to the map sizes, which is the
    // number duplicates would quietly shrink.
    expect(inv.all).toHaveLength(inv.closed.size + inv.open.size + inv.unclassified.length);
  });

  it('actually reads the repo and finds citations in it', () => {
    // If `git ls-files` or the citation pattern breaks, this is what goes red
    // rather than the whole check quietly passing.
    expect(files.length).toBeGreaterThanOrEqual(MIN_SCANNED_FILES);
    expect(stats.citations).toBeGreaterThanOrEqual(MIN_CITATIONS);
    expect(stats.pathRefs).toBeGreaterThanOrEqual(MIN_PATH_REFS);
  });

  it('writes citations in BOTH forms, each clear of its own floor', () => {
    // If either form ever genuinely falls out of use, this goes red and the
    // floor gets retired deliberately rather than the guard quietly covering
    // a form nobody writes any more.
    expect(stats.byForm.hyphen).toBeGreaterThanOrEqual(MIN_HYPHEN_CITATIONS);
    expect(stats.byForm.spaced).toBeGreaterThanOrEqual(MIN_SPACED_CITATIONS);
    expect(stats.byForm.hyphen + stats.byForm.spaced).toBe(stats.citations);
  });

  it('resolves the collision pair both ways, proving padding is what separates them', () => {
    // Concrete anchor rather than a derived comparison: 13 must exist in both
    // series and name two DIFFERENT records.
    expect(inv.closed.get('0013')).toBe('0013-email-confirmation-watching.md');
    expect(inv.open.get('013')).toBe('adr-013-resume-builder-base-plus-handoff.md');
  });

  it('has every citation and path reference resolving', () => {
    expect(violations(stats, inv, files.length)).toEqual([]);
  });
});
