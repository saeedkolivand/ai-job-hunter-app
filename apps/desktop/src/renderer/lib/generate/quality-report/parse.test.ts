import { describe, expect, it } from 'vitest';

import { hashText, parseQualityReport, serializeQualityReport } from '../quality-report';
import { OK_REPORT, QUALITY_WRAPPER } from './test-support';

describe('parseQualityReport', () => {
  it('returns null for undefined, empty, and the Rust-side {} placeholder', () => {
    expect(parseQualityReport(undefined)).toBeNull();
    expect(parseQualityReport('')).toBeNull();
    expect(parseQualityReport('{}')).toBeNull();
  });

  it('returns null for unparseable JSON instead of throwing', () => {
    expect(parseQualityReport('not json')).toBeNull();
  });

  it('round-trips a real report', () => {
    const slot = { report: OK_REPORT, sourceTextHash: hashText('generated resume') };
    const raw = JSON.stringify({
      schemaVersion: 2,
      pipeline: 'fast',
      generatedAt: 123,
      resume: slot,
    });
    expect(parseQualityReport(raw)).toEqual({
      schemaVersion: 2,
      pipeline: 'fast',
      generatedAt: 123,
      resume: slot,
    });
  });

  // v1 kept the hashes in a sibling map next to top-level sub-reports, which the
  // per-top-level-key persistence merge could strip out from under a surviving
  // sub-report. It is rejected outright rather than migrated: such a blob only
  // exists on a machine that ran this feature branch pre-slot, and "no report
  // until the next generation" is the correct (never falsely-green) degrade.
  it('rejects a v1-shaped blob (top-level sub-reports + a sibling sourceTextHash map)', () => {
    const raw = JSON.stringify({
      schemaVersion: 1,
      pipeline: 'fast',
      generatedAt: 123,
      resume: OK_REPORT,
      coverLetter: OK_REPORT,
      sourceTextHash: { resume: hashText('r'), coverLetter: hashText('c') },
    });
    expect(() => parseQualityReport(raw)).not.toThrow();
    expect(parseQualityReport(raw)).toBeNull();
  });

  it('preserves a null topRequirementHits through a reopen — "not measured" never rehydrates as 0', () => {
    const unmeasured = {
      ok: true,
      issues: [],
      metrics: { ...OK_REPORT.metrics, topRequirementHits: null, topRequirementsMeasured: null },
    };
    const raw = JSON.stringify({
      schemaVersion: 2,
      pipeline: 'fast',
      generatedAt: 1,
      resume: { report: unmeasured, sourceTextHash: 9 },
    });
    // Coercing null to 0 here would render "Top requirements covered: 0" as a
    // fact on cold entry — the exact state the Option<u32> wire removed.
    const metrics = parseQualityReport(raw)?.resume?.report.metrics;
    expect(metrics?.topRequirementHits).toBeNull();
    expect(metrics?.topRequirementsMeasured).toBeNull();
  });

  it('carries the requirement denominator through a reopen', () => {
    const measured = {
      ok: true,
      issues: [],
      metrics: { ...OK_REPORT.metrics, topRequirementHits: 2, topRequirementsMeasured: 4 },
    };
    const raw = JSON.stringify({
      schemaVersion: 2,
      pipeline: 'fast',
      generatedAt: 1,
      resume: { report: measured, sourceTextHash: 9 },
    });
    // Dropping the denominator at the parse layer would strand a reopened
    // report with an uninterpretable bare count (round-9 finding).
    expect(parseQualityReport(raw)?.resume?.report.metrics.topRequirementsMeasured).toBe(4);
  });

  it('drops a slot that carries a report but no hash — never a report without its anchor', () => {
    const raw = JSON.stringify({
      schemaVersion: 2,
      pipeline: 'fast',
      generatedAt: 1,
      resume: { report: OK_REPORT },
      coverLetter: { report: OK_REPORT, sourceTextHash: 42 },
    });
    const result = parseQualityReport(raw);
    expect(result?.resume).toBeUndefined();
    expect(result?.coverLetter).toEqual({ report: OK_REPORT, sourceTextHash: 42 });
  });

  // Security finding M-1: these are the exact malformed persisted shapes that
  // crashed the whole app (only the root ErrorBoundary caught it) — the cast
  // in `parseQualityReport` skipped shape validation entirely. None of these
  // may ever throw; a malformed report degrades to `null`.
  describe('malformed persisted reports never throw (M-1)', () => {
    const malformed = [
      ['non-array issues (a number)', '{"resume":{"issues":42}}'],
      ['a boolean in place of the sub-report object', '{"resume":true}'],
      ['a plain object instead of an issues array', '{"resume":{"issues":{"a":1}}}'],
      ['a string instead of an issues array', '{"resume":{"issues":"abc"}}'],
    ] as const;

    it.each(malformed)('returns null, never throws, for: %s', (_desc, raw) => {
      expect(() => parseQualityReport(raw)).not.toThrow();
      expect(parseQualityReport(raw)).toBeNull();
    });

    it('drops just the malformed resume slot when schemaVersion is present and valid', () => {
      const raw = JSON.stringify({
        schemaVersion: 2,
        pipeline: 'fast',
        generatedAt: 1,
        resume: { report: { issues: 42 }, sourceTextHash: 1 },
      });
      const result = parseQualityReport(raw);
      expect(() => parseQualityReport(raw)).not.toThrow();
      expect(result?.resume).toBeUndefined();
    });

    it('drops a sub-report whose issues array contains a malformed entry, keeping the valid entries', () => {
      const raw = JSON.stringify({
        schemaVersion: 2,
        pipeline: 'fast',
        generatedAt: 1,
        resume: {
          sourceTextHash: 7,
          report: {
            ok: true,
            issues: [
              {
                severity: 'critical',
                code: 'factual.dropped_role',
                section: null,
                message: 'ok entry',
                evidence: null,
              },
              { severity: 'nonsense', code: 123 },
            ],
            metrics: OK_REPORT.metrics,
          },
        },
      });
      const result = parseQualityReport(raw);
      expect(result?.resume?.report.issues).toEqual([
        {
          severity: 'critical',
          code: 'factual.dropped_role',
          section: null,
          message: 'ok entry',
          evidence: null,
        },
      ]);
    });

    it('treats schemaVersion 3 as absent (forward-compatible, not pattern-matched against v2 fields)', () => {
      const raw = JSON.stringify({
        schemaVersion: 3,
        pipeline: 'fast',
        generatedAt: 1,
        resume: { report: OK_REPORT, sourceTextHash: 1 },
      });
      expect(() => parseQualityReport(raw)).not.toThrow();
      expect(parseQualityReport(raw)).toBeNull();
    });

    it('keeps a valid resume slot while dropping a malformed cover letter slot', () => {
      const raw = JSON.stringify({
        schemaVersion: 2,
        pipeline: 'fast',
        generatedAt: 1,
        resume: { report: OK_REPORT, sourceTextHash: 5 },
        coverLetter: { report: { issues: 42 }, sourceTextHash: 6 },
      });
      const result = parseQualityReport(raw);
      expect(result?.resume).toEqual({ report: OK_REPORT, sourceTextHash: 5 });
      expect(result?.coverLetter).toBeUndefined();
    });
  });
});

/**
 * Both surfaces share ONE `ai_generations` row per job url, and "Re-check" is a
 * full round-trip over it: parse → merge → serialize → save (a merge-upsert
 * that overlays per top-level key). So anything this parser fails to carry is
 * not merely invisible — it is DELETED from the record on the next re-check.
 */
describe('parse → serialize round-trip of a staged-pipeline wrapper', () => {
  const raw = JSON.stringify(QUALITY_WRAPPER);

  it('preserves the depth — a quality run must never re-read as fast', () => {
    expect(parseQualityReport(raw)?.pipeline).toBe('quality');
  });

  it('preserves fabrications and their mixed resolved/unresolved verdicts, byte for byte', () => {
    const round = String(serializeQualityReport(parseQualityReport(raw)));

    // Byte-level: the fabrication list is re-serialized exactly as it arrived,
    // including the resolved entry's `decision` and the unresolved entry's
    // ABSENT one (an invented `decision` would silently resolve a finding).
    expect(round).toContain(JSON.stringify(QUALITY_WRAPPER.resume?.fabrications));
    // …and the wrapper as a whole survives unchanged.
    expect(JSON.parse(round)).toEqual(QUALITY_WRAPPER);
  });

  it('overlays the VALIDATED report onto the source slot — the passthrough never resurrects a raw one', () => {
    const withBadIssue = JSON.stringify({
      schemaVersion: 2,
      pipeline: 'quality',
      generatedAt: 1,
      resume: {
        sourceTextHash: 7,
        fabrications: QUALITY_WRAPPER.resume?.fabrications,
        report: {
          ok: true,
          issues: [
            {
              severity: 'critical',
              code: 'factual.dropped_role',
              section: null,
              message: 'ok entry',
              evidence: null,
            },
            { severity: 'nonsense', code: 123 },
          ],
          metrics: OK_REPORT.metrics,
        },
      },
    });

    const parsed = parseQualityReport(withBadIssue);

    // Carrying unknown keys must not carry the UNVALIDATED report with them:
    // the malformed issue is still dropped (M-1), the fabrications still ride.
    expect(parsed?.resume?.report.issues).toHaveLength(1);
    expect(String(serializeQualityReport(parsed))).not.toContain('nonsense');
    expect(String(serializeQualityReport(parsed))).toContain('Led a team of 12');
  });

  it('reads a v2 wrapper written before the pipeline existed (no depth field) as fast', () => {
    const noPipeline = JSON.stringify({
      schemaVersion: 2,
      generatedAt: 1,
      resume: { report: OK_REPORT, sourceTextHash: 3 },
    });
    expect(parseQualityReport(noPipeline)?.pipeline).toBe('fast');
  });

  it('degrades an unrecognised depth to fast rather than carrying a value nothing can render', () => {
    const bogus = JSON.stringify({ ...QUALITY_WRAPPER, pipeline: 'turbo' });
    expect(parseQualityReport(bogus)?.pipeline).toBe('fast');
  });

  it('still reads a historic `max`-depth wrapper as `max`, not as an unrecognised value', () => {
    // PR-4 removed the `max` generation depth — no run can produce this label
    // any more — but an EXISTING row saved before the removal still has one,
    // and there is no migration touching old rows. `max` must stay a member of
    // the shared depth vocabulary (`GENERATION_DEPTHS`) purely for THIS read
    // path, or every pre-existing max-depth report silently relabels itself
    // `fast` on the next open — the exact bug `parsePipeline`'s fallback
    // exists to avoid for a genuinely unrecognised value, applied wrongly to a
    // value that is still recognised.
    const historicMax = JSON.stringify({ ...QUALITY_WRAPPER, pipeline: 'max' });
    expect(parseQualityReport(historicMax)?.pipeline).toBe('max');
  });
});
