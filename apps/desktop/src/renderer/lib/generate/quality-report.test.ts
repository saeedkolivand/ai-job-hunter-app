import { describe, expect, it, vi } from 'vitest';

import type { ContentReportPayload, PipelineQualityReport } from '@ajh/shared/ipc';

import { _registerClient } from '../app-client';
import { createMockClient } from '../mock-client';
import {
  computeQualityReport,
  hashText,
  mergeRecheckedReport,
  parseQualityReport,
  serializeQualityReport,
} from './quality-report';

const OK_REPORT: ContentReportPayload = {
  ok: true,
  issues: [],
  metrics: {
    keywordCoverage: 80,
    topRequirementHits: 2,
    topRequirementsMeasured: 4,
    duplicateRatio: 0,
    rolesSource: 3,
    rolesOutput: 3,
  },
};

const CRITICAL_REPORT: ContentReportPayload = {
  ok: false,
  issues: [
    {
      severity: 'critical',
      code: 'factual.dropped_role',
      section: 'Experience',
      message: 'A role from the source résumé is missing.',
      evidence: 'Acme Corp',
    },
  ],
  metrics: {
    keywordCoverage: 40,
    topRequirementHits: 0,
    topRequirementsMeasured: 0,
    duplicateRatio: 0,
    rolesSource: 3,
    rolesOutput: 2,
  },
};

/**
 * What the staged Rust pipeline persists into the SAME `ai_generations` row the
 * fast path reads and rewrites: depth `'quality'`, plus a slot carrying the
 * per-bullet fabrication review — one bullet the user already ruled on, one
 * still awaiting a verdict (which is what holds the run at `needsReview`).
 */
const QUALITY_WRAPPER: PipelineQualityReport = {
  schemaVersion: 2,
  pipeline: 'quality',
  generatedAt: 1700,
  resume: {
    report: CRITICAL_REPORT,
    sourceTextHash: hashText('generated resume'),
    fabrications: [
      {
        issueKey: 'factual.unsupported_metric#0',
        code: 'factual.unsupported_metric',
        evidence: 'Cut p99 latency by 60%',
        decision: 'keep',
      },
      {
        issueKey: 'factual.unsupported_metric#1',
        code: 'factual.unsupported_metric',
        evidence: 'Led a team of 12',
      },
    ],
  },
  coverLetter: { report: OK_REPORT, sourceTextHash: hashText('generated cover') },
};

function register(validateContent: ReturnType<typeof vi.fn>) {
  _registerClient(createMockClient({ resume: { validateContent } }));
}

describe('computeQualityReport', () => {
  it('returns null when neither doc was generated', async () => {
    register(vi.fn());
    const report = await computeQualityReport({
      sourceResume: 'src',
      jobAd: 'ad',
      topRequirements: [],
      targetLanguage: 'en',
    });
    expect(report).toBeNull();
  });

  it('validates only the résumé when no cover letter was generated', async () => {
    const validateContent = vi.fn().mockResolvedValue(OK_REPORT);
    register(validateContent);

    const report = await computeQualityReport({
      sourceResume: 'src',
      jobAd: 'ad',
      topRequirements: ['req'],
      targetLanguage: 'en',
      resumeText: 'generated resume',
    });

    expect(validateContent).toHaveBeenCalledTimes(1);
    expect(validateContent).toHaveBeenCalledWith(
      expect.objectContaining({ generated: 'generated resume', docKind: 'resume' })
    );
    expect(report).toEqual(
      expect.objectContaining({
        schemaVersion: 2,
        pipeline: 'fast',
        resume: { report: OK_REPORT, sourceTextHash: hashText('generated resume') },
      })
    );
    expect(report?.coverLetter).toBeUndefined();
  });

  // The persistence merge (Rust `merge_quality_report`) overlays per TOP-LEVEL
  // key, so a key this run cannot fill must be ABSENT, not present-and-empty:
  // a résumé-only regeneration that carried a `coverLetter` key would overwrite
  // the stored letter slot — hash included — and strip its staleness anchor.
  it('carries NO cover-letter key at all on a résumé-only run (the merge can only drop what it carries)', async () => {
    register(vi.fn().mockResolvedValue(OK_REPORT));

    const report = await computeQualityReport({
      sourceResume: 'src',
      jobAd: 'ad',
      topRequirements: [],
      targetLanguage: 'en',
      resumeText: 'generated resume',
    });

    expect(report).not.toBeNull();
    expect(Object.keys(report ?? {})).not.toContain('coverLetter');
    // …and the surviving key is a self-contained slot: verdict + its own hash.
    expect(report?.resume).toEqual({
      report: OK_REPORT,
      sourceTextHash: hashText('generated resume'),
    });
    // The serialized blob the merge actually sees carries no letter key either.
    expect(JSON.parse(JSON.stringify(report))).not.toHaveProperty('coverLetter');
  });

  it('validates both docs in parallel and carries a critical report through', async () => {
    const validateContent = vi
      .fn()
      .mockImplementation(async (req: { docKind: 'resume' | 'coverLetter' }) =>
        req.docKind === 'resume' ? CRITICAL_REPORT : OK_REPORT
      );
    register(validateContent);

    const report = await computeQualityReport({
      sourceResume: 'src',
      jobAd: 'ad',
      topRequirements: [],
      targetLanguage: 'en',
      resumeText: 'r',
      coverLetterText: 'c',
    });

    expect(validateContent).toHaveBeenCalledTimes(2);
    expect(report?.resume?.report).toEqual(CRITICAL_REPORT);
    expect(report?.coverLetter?.report).toEqual(OK_REPORT);
  });

  it('degrades to no report for a doc whose validation call fails, never throwing', async () => {
    const validateContent = vi.fn().mockRejectedValue(new Error('boom'));
    register(validateContent);

    const report = await computeQualityReport({
      sourceResume: 'src',
      jobAd: 'ad',
      topRequirements: [],
      targetLanguage: 'en',
      resumeText: 'r',
    });

    // Neither doc validated successfully — the whole report degrades to null,
    // never a thrown error the caller would have to catch.
    expect(report).toBeNull();
  });

  it('keeps the successful doc when the other one fails', async () => {
    const validateContent = vi
      .fn()
      .mockImplementation(async (req: { docKind: 'resume' | 'coverLetter' }) => {
        if (req.docKind === 'coverLetter') throw new Error('cover validation boom');
        return OK_REPORT;
      });
    register(validateContent);

    const report = await computeQualityReport({
      sourceResume: 'src',
      jobAd: 'ad',
      topRequirements: [],
      targetLanguage: 'en',
      resumeText: 'r',
      coverLetterText: 'c',
    });

    expect(report?.resume?.report).toEqual(OK_REPORT);
    expect(report?.coverLetter).toBeUndefined();
  });

  it("hashes each validated doc's EXACT text into its own slot", async () => {
    const validateContent = vi
      .fn()
      .mockImplementation(async (req: { docKind: 'resume' | 'coverLetter' }) =>
        req.docKind === 'resume' ? OK_REPORT : CRITICAL_REPORT
      );
    register(validateContent);

    const report = await computeQualityReport({
      sourceResume: 'src',
      jobAd: 'ad',
      topRequirements: [],
      targetLanguage: 'en',
      resumeText: 'generated resume',
      coverLetterText: 'generated cover',
    });

    expect(report?.resume?.sourceTextHash).toBe(hashText('generated resume'));
    expect(report?.coverLetter?.sourceTextHash).toBe(hashText('generated cover'));
  });

  it("omits a doc's slot entirely when that doc never validated (no hash without a report)", async () => {
    const validateContent = vi.fn().mockResolvedValue(OK_REPORT);
    register(validateContent);

    const report = await computeQualityReport({
      sourceResume: 'src',
      jobAd: 'ad',
      topRequirements: [],
      targetLanguage: 'en',
      resumeText: 'generated resume',
    });

    expect(report?.resume?.sourceTextHash).toBe(hashText('generated resume'));
    expect(report?.coverLetter).toBeUndefined();
  });
});

describe('hashText', () => {
  it('is stable for the same input', () => {
    expect(hashText('hello world')).toBe(hashText('hello world'));
  });

  it('differs for different input', () => {
    expect(hashText('hello world')).not.toBe(hashText('hello world!'));
  });

  it('never returns a negative number (unsigned 32-bit)', () => {
    expect(hashText('x'.repeat(5000))).toBeGreaterThanOrEqual(0);
  });
});

describe('mergeRecheckedReport', () => {
  it('replaces only the rechecked doc, leaving the other slot (report AND hash) intact', () => {
    const existing = {
      schemaVersion: 2 as const,
      pipeline: 'fast' as const,
      generatedAt: 111,
      resume: { report: CRITICAL_REPORT, sourceTextHash: hashText('old resume') },
      coverLetter: { report: OK_REPORT, sourceTextHash: hashText('old cover') },
    };

    const merged = mergeRecheckedReport(existing, 'resume', OK_REPORT, 'new resume');

    expect(merged.resume).toEqual({ report: OK_REPORT, sourceTextHash: hashText('new resume') });
    // Untouched — verdict and hash travel together, so neither can be orphaned.
    expect(merged.coverLetter).toEqual({
      report: OK_REPORT,
      sourceTextHash: hashText('old cover'),
    });
    expect(merged.generatedAt).toBe(111); // untouched
  });

  it('builds a fresh wrapper when there is no existing report', () => {
    const merged = mergeRecheckedReport(null, 'coverLetter', OK_REPORT, 'cover text');

    expect(merged.schemaVersion).toBe(2);
    expect(merged.coverLetter).toEqual({
      report: OK_REPORT,
      sourceTextHash: hashText('cover text'),
    });
    expect(merged.resume).toBeUndefined();
    // No prior wrapper: this re-check IS the fast deterministic pass.
    expect(merged.pipeline).toBe('fast');
  });

  // A re-check produces a verdict and a hash — nothing else. Everything else in
  // the slot is review state the user (and the Rust pipeline) own: rebuilding
  // the slot here erases it from the persisted record, after which
  // `resolveFabrication` no-ops and the run is stuck `needsReview` forever.
  it('replaces only the verdict+hash of the rechecked slot, keeping its fabrications and verdicts', () => {
    const merged = mergeRecheckedReport(QUALITY_WRAPPER, 'resume', OK_REPORT, 'new resume');

    expect(merged.resume).toEqual({
      ...QUALITY_WRAPPER.resume,
      report: OK_REPORT,
      sourceTextHash: hashText('new resume'),
    });
    expect(merged.coverLetter).toEqual(QUALITY_WRAPPER.coverLetter);
  });

  it('keeps the wrapper depth — a re-check never relabels a quality run as fast', () => {
    expect(mergeRecheckedReport(QUALITY_WRAPPER, 'resume', OK_REPORT, 'new resume').pipeline).toBe(
      'quality'
    );
  });
});

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
