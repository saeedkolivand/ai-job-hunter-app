import { describe, expect, it, vi } from 'vitest';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import { _registerClient } from '../app-client';
import { createMockClient } from '../mock-client';
import { computeQualityReport, parseQualityReport } from './quality-report';

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
});
