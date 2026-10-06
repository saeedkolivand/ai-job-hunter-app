import { describe, expect, it, vi } from 'vitest';

import { computeQualityReport, hashText, mergeRecheckedReport } from '../quality-report';
import { CRITICAL_REPORT, OK_REPORT, QUALITY_WRAPPER, register } from './test-support';

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
