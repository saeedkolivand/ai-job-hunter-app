import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import type { GenerationMeta, QualityReport } from '@/lib/generate';

import { useQualityRecheck } from './use-quality-recheck';

// The two service hooks reach for AppClientProvider — stub both. `mutateAsync`
// is replaced per test so a re-check can be held in flight while the session
// moves on underneath it.
const validateContentMock = vi.hoisted(() => ({
  mutateAsync: vi.fn(),
  isPending: false,
}));
vi.mock('@/services/use-resume-validation', () => ({
  useValidateContent: () => validateContentMock,
}));

const saveGenerationMock = vi.hoisted(() => ({ mutate: vi.fn() }));
vi.mock('@/services/use-ai-generations', () => ({
  useSaveAiGeneration: () => saveGenerationMock,
}));

const META: GenerationMeta = {
  candidateName: 'Ada',
  jobTitle: 'Engineer',
  companyName: 'Acme',
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  targetLanguage: 'en',
  topRequirements: ['rust'],
};

const RESUME = 'Led the payments migration.';
const COVER = 'Dear Team, ...';

function payload(keywordCoverage: number): ContentReportPayload {
  return {
    ok: true,
    issues: [],
    metrics: {
      keywordCoverage,
      topRequirementHits: 1,
      duplicateRatio: 0,
      rolesSource: 1,
      rolesOutput: 1,
    },
  };
}

/** A slot for the OTHER document, used to prove a re-check never drops it. */
const COVER_SLOT = { report: payload(10), sourceTextHash: 42 };

type Params = Parameters<typeof useQualityRecheck>[0];

function baseParams(overrides: Partial<Params> = {}): Params {
  return {
    report: null,
    meta: META,
    sourceResume: 'source resume text',
    jobAd: 'the job ad',
    docKind: 'resume',
    onReportChange: vi.fn(),
    resumeText: RESUME,
    coverLetterText: COVER,
    generating: false,
    jobUrl: 'https://acme.com/job/42',
    board: 'linkedin',
    ...overrides,
  };
}

/** Hold `mutateAsync` pending, returning its resolver. */
function deferValidation() {
  let resolve!: (payload: ContentReportPayload) => void;
  validateContentMock.mutateAsync = vi.fn(
    () =>
      new Promise<ContentReportPayload>((r) => {
        resolve = r;
      })
  );
  return (value: ContentReportPayload) => resolve(value);
}

/** Let the post-await half of `handleRecheck` run to completion. */
async function flushMicrotasks() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

describe('useQualityRecheck', () => {
  beforeEach(() => {
    validateContentMock.mutateAsync = vi.fn();
    validateContentMock.isPending = false;
    saveGenerationMock.mutate.mockClear();
  });

  // The guard that Regenerate defeats when it is keyed off input identity:
  // `handleGenerate`/`runTailor` re-run with the SAME meta/source/job ad, so
  // only the run signal itself can invalidate the in-flight result. The texts
  // are deliberately left UNCHANGED here so nothing but the epoch can block it.
  it('drops a resolution that lands after a new run started (Regenerate)', async () => {
    const resolveValidation = deferValidation();
    const onReportChange = vi.fn();
    const { result, rerender } = renderHook((props: Params) => useQualityRecheck(props), {
      initialProps: baseParams({ onReportChange }),
    });

    act(() => result.current.recheck?.());
    // Regenerate: same inputs, same text, only the run flag flips.
    rerender(baseParams({ onReportChange, generating: true }));
    resolveValidation(payload(90));
    await flushMicrotasks();

    expect(onReportChange).not.toHaveBeenCalled();
    expect(saveGenerationMock.mutate).not.toHaveBeenCalled();
  });

  // Reset replaces the identity-defining inputs without ever setting
  // `generating` — the original guard, kept.
  it('drops a resolution that lands after the inputs were replaced (Reset)', async () => {
    const resolveValidation = deferValidation();
    const onReportChange = vi.fn();
    const { result, rerender } = renderHook((props: Params) => useQualityRecheck(props), {
      initialProps: baseParams({ onReportChange }),
    });

    act(() => result.current.recheck?.());
    rerender(baseParams({ onReportChange, meta: null, sourceResume: '', jobAd: '' }));
    resolveValidation(payload(90));
    await flushMicrotasks();

    expect(onReportChange).not.toHaveBeenCalled();
    expect(saveGenerationMock.mutate).not.toHaveBeenCalled();
  });

  it('merges the fresh slot and persists it with the texts the new hash describes', async () => {
    const fresh = payload(90);
    validateContentMock.mutateAsync = vi.fn().mockResolvedValue(fresh);
    const onReportChange = vi.fn();
    const { result } = renderHook((props: Params) => useQualityRecheck(props), {
      initialProps: baseParams({ onReportChange }),
    });

    act(() => result.current.recheck?.());
    await flushMicrotasks();

    expect(validateContentMock.mutateAsync).toHaveBeenCalledWith({
      generated: RESUME,
      source: 'source resume text',
      jobAd: 'the job ad',
      topRequirements: ['rust'],
      targetLanguage: 'en',
      docKind: 'resume',
    });

    const merged = onReportChange.mock.calls[0]?.[0] as QualityReport;
    expect(merged.resume?.report).toBe(fresh);

    expect(saveGenerationMock.mutate).toHaveBeenCalledTimes(1);
    const request = saveGenerationMock.mutate.mock.calls[0]?.[0];
    // The texts persisted are exactly the ones the fresh hash was computed
    // over — a save that carried superseded text would silently revert the
    // record (the Rust merge takes any non-blank incoming text).
    expect(request.resumeText).toBe(RESUME);
    expect(request.coverLetterText).toBe(COVER);
    expect(JSON.parse(String(request.qualityReport))).toEqual(
      expect.objectContaining({ schemaVersion: 2, resume: merged.resume })
    );
    expect(request.jobUrl).toBe('https://acme.com/job/42');
    expect(request.board).toBe('linkedin');
    // Everything else blank so the Rust-side merge keeps the stored value.
    expect(request.candidateName).toBe('');
    expect(request.jobAd).toBe('');
    expect(request.mode).toBe('');
    expect(request.topRequirements).toEqual([]);
  });

  it('keeps the re-check session-only when there is no jobUrl to merge onto', async () => {
    validateContentMock.mutateAsync = vi.fn().mockResolvedValue(payload(90));
    const onReportChange = vi.fn();
    const { result } = renderHook((props: Params) => useQualityRecheck(props), {
      initialProps: baseParams({ onReportChange, jobUrl: undefined }),
    });

    act(() => result.current.recheck?.());
    await flushMicrotasks();

    // The session still gets the merged report; only the write is skipped —
    // a url-less save would INSERT a duplicate row instead of merging.
    expect(onReportChange).toHaveBeenCalledTimes(1);
    expect(saveGenerationMock.mutate).not.toHaveBeenCalled();
  });

  // Two re-checks of different documents overlap: the letter's slot lands
  // while the résumé's check is still in flight. Merging onto the closure's
  // (now superseded) base would silently drop the letter again.
  it('merges onto the CURRENT base report, keeping a slot added while it was in flight', async () => {
    const resolveValidation = deferValidation();
    const onReportChange = vi.fn();
    const { result, rerender } = renderHook((props: Params) => useQualityRecheck(props), {
      initialProps: baseParams({ onReportChange, report: null }),
    });

    act(() => result.current.recheck?.());
    // The cover-letter re-check resolved first and wrote its slot back.
    const withCover: QualityReport = {
      schemaVersion: 2,
      pipeline: 'fast',
      generatedAt: 7,
      coverLetter: COVER_SLOT,
    };
    rerender(baseParams({ onReportChange, report: withCover }));
    resolveValidation(payload(90));
    await flushMicrotasks();

    const merged = onReportChange.mock.calls[0]?.[0] as QualityReport;
    expect(merged.coverLetter).toEqual(COVER_SLOT);
    expect(merged.resume?.report.metrics.keywordCoverage).toBe(90);
  });

  it('drops a resolution whose document text changed while the check ran', async () => {
    const resolveValidation = deferValidation();
    const onReportChange = vi.fn();
    const { result, rerender } = renderHook((props: Params) => useQualityRecheck(props), {
      initialProps: baseParams({ onReportChange }),
    });

    act(() => result.current.recheck?.());
    rerender(baseParams({ onReportChange, resumeText: `${RESUME} And more.` }));
    resolveValidation(payload(90));
    await flushMicrotasks();

    // Merging here would attach a hash describing text the document no longer
    // holds — a permanently, wrongly "fresh" badge.
    expect(onReportChange).not.toHaveBeenCalled();
    expect(saveGenerationMock.mutate).not.toHaveBeenCalled();
  });

  it('leaves the report untouched when validation fails (best-effort)', async () => {
    validateContentMock.mutateAsync = vi.fn().mockRejectedValue(new Error('boom'));
    const onReportChange = vi.fn();
    const { result } = renderHook((props: Params) => useQualityRecheck(props), {
      initialProps: baseParams({ onReportChange }),
    });

    act(() => result.current.recheck?.());
    await flushMicrotasks();

    expect(onReportChange).not.toHaveBeenCalled();
    expect(saveGenerationMock.mutate).not.toHaveBeenCalled();
  });

  it('hides the action entirely when no session writer is wired', () => {
    const { result } = renderHook((props: Params) => useQualityRecheck(props), {
      initialProps: baseParams({ onReportChange: undefined }),
    });

    expect(result.current.recheck).toBeUndefined();
  });
});
