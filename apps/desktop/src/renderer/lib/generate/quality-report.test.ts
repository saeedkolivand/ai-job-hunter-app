import { describe, expect, it, vi } from 'vitest';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import { _registerClient } from '../app-client';
import { createMockClient } from '../mock-client';
import {
  computeQualityReport,
  hashText,
  mergeRecheckedReport,
  parseQualityReport,
} from './quality-report';

const OK_REPORT: ContentReportPayload = {
  ok: true,
  issues: [],
  metrics: {
    keywordCoverage: 80,
    topRequirementHits: 2,
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
    duplicateRatio: 0,
    rolesSource: 3,
    rolesOutput: 2,
  },
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
      expect.objectContaining({ schemaVersion: 1, pipeline: 'fast', resume: OK_REPORT })
    );
    expect(report?.coverLetter).toBeUndefined();
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
    expect(report?.resume).toEqual(CRITICAL_REPORT);
    expect(report?.coverLetter).toEqual(OK_REPORT);
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

    expect(report?.resume).toEqual(OK_REPORT);
    expect(report?.coverLetter).toBeUndefined();
  });

  it("hashes each validated doc's EXACT text into sourceTextHash", async () => {
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

    expect(report?.sourceTextHash).toEqual({
      resume: hashText('generated resume'),
      coverLetter: hashText('generated cover'),
    });
  });

  it("omits a doc's hash when that doc never validated (no crash, no bogus entry)", async () => {
    const validateContent = vi.fn().mockResolvedValue(OK_REPORT);
    register(validateContent);

    const report = await computeQualityReport({
      sourceResume: 'src',
      jobAd: 'ad',
      topRequirements: [],
      targetLanguage: 'en',
      resumeText: 'generated resume',
    });

    expect(report?.sourceTextHash).toEqual({ resume: hashText('generated resume') });
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
  it('replaces only the rechecked doc, leaving the other sub-report + hash intact', () => {
    const existing = {
      schemaVersion: 1 as const,
      pipeline: 'fast' as const,
      generatedAt: 111,
      resume: CRITICAL_REPORT,
      coverLetter: OK_REPORT,
      sourceTextHash: { resume: hashText('old resume'), coverLetter: hashText('old cover') },
    };

    const merged = mergeRecheckedReport(existing, 'resume', OK_REPORT, 'new resume');

    expect(merged.resume).toEqual(OK_REPORT);
    expect(merged.coverLetter).toEqual(OK_REPORT); // untouched
    expect(merged.sourceTextHash).toEqual({
      resume: hashText('new resume'),
      coverLetter: hashText('old cover'), // untouched
    });
    expect(merged.generatedAt).toBe(111); // untouched
  });

  it('builds a fresh wrapper when there is no existing report', () => {
    const merged = mergeRecheckedReport(null, 'coverLetter', OK_REPORT, 'cover text');

    expect(merged.coverLetter).toEqual(OK_REPORT);
    expect(merged.resume).toBeUndefined();
    expect(merged.sourceTextHash).toEqual({ coverLetter: hashText('cover text') });
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
    const raw = JSON.stringify({
      schemaVersion: 1,
      pipeline: 'fast',
      generatedAt: 123,
      resume: OK_REPORT,
    });
    expect(parseQualityReport(raw)).toEqual({
      schemaVersion: 1,
      pipeline: 'fast',
      generatedAt: 123,
      resume: OK_REPORT,
    });
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

    it('drops just the malformed resume sub-report when schemaVersion is present and valid', () => {
      const raw = JSON.stringify({
        schemaVersion: 1,
        pipeline: 'fast',
        generatedAt: 1,
        resume: { issues: 42 },
      });
      const result = parseQualityReport(raw);
      expect(() => parseQualityReport(raw)).not.toThrow();
      expect(result?.resume).toBeUndefined();
    });

    it('drops a sub-report whose issues array contains a malformed entry, keeping the valid entries', () => {
      const raw = JSON.stringify({
        schemaVersion: 1,
        pipeline: 'fast',
        generatedAt: 1,
        resume: {
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
      });
      const result = parseQualityReport(raw);
      expect(result?.resume?.issues).toEqual([
        {
          severity: 'critical',
          code: 'factual.dropped_role',
          section: null,
          message: 'ok entry',
          evidence: null,
        },
      ]);
    });

    it('treats schemaVersion 2 as absent (forward-compatible, not pattern-matched against v1 fields)', () => {
      const raw = JSON.stringify({
        schemaVersion: 2,
        pipeline: 'fast',
        generatedAt: 1,
        resume: OK_REPORT,
      });
      expect(() => parseQualityReport(raw)).not.toThrow();
      expect(parseQualityReport(raw)).toBeNull();
    });

    it('keeps a valid resume report while dropping a malformed cover letter report', () => {
      const raw = JSON.stringify({
        schemaVersion: 1,
        pipeline: 'fast',
        generatedAt: 1,
        resume: OK_REPORT,
        coverLetter: { issues: 42 },
      });
      const result = parseQualityReport(raw);
      expect(result?.resume).toEqual(OK_REPORT);
      expect(result?.coverLetter).toBeUndefined();
    });
  });
});
