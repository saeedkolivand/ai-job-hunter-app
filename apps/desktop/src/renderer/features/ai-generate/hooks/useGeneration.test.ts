import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import {
  computeQualityReport,
  generateCoverLetter,
  generateResume,
  type GenerationMeta,
} from '@/lib/generate';

import { useGeneration } from './useGeneration';

// Stub the generation pipeline. extractMetadata resolves a minimal meta; the
// résumé/cover generators emit one token and return a fixed string by default.
vi.mock('@/lib/generate', () => ({
  extractMetadata: vi.fn().mockResolvedValue({
    candidateName: 'A',
    jobTitle: 'Dev',
    companyName: 'Co',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
    topRequirements: [],
  }),
  generateResume: vi.fn(async (..._a: unknown[]) => 'RESUME'),
  generateCoverLetter: vi.fn(async (..._a: unknown[]) => ({
    text: 'COVER',
    companyBrief: 'BRIEF',
  })),
  computeQualityReport: vi.fn().mockResolvedValue(null),
  serializeQualityReport: vi.fn((r: unknown) => (r ? JSON.stringify(r) : undefined)),
}));

const META: GenerationMeta = {
  candidateName: 'A',
  jobTitle: 'Dev',
  companyName: 'Co',
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  targetLanguage: 'en',
  topRequirements: [],
};

type Target = 'resume' | 'cover' | 'both';

/**
 * Build the (large, positional) useGeneration arg list with vi.fn() setters.
 * `useGeneration` is a plain factory (no React hooks inside), but it is named
 * like a hook, so it is invoked via `renderHook` to satisfy rules-of-hooks.
 */
function setup(target: Target, provenance?: { jobUrl?: string; board?: string }) {
  const m = {
    setStage: vi.fn(),
    setMeta: vi.fn(),
    setReport: vi.fn(),
    setResumeOut: vi.fn(),
    setCoverOut: vi.fn(),
    setActiveOut: vi.fn(),
    setStreamBuffer: vi.fn(),
    setThinkingBuffer: vi.fn(),
    setModelLoading: vi.fn(),
    setTokenCount: vi.fn(),
    setGenStep: vi.fn(),
    setError: vi.fn(),
    startStageRotation: vi.fn(),
    stopStageRotation: vi.fn(),
    saveAiGeneration: { mutate: vi.fn() },
    setStageLabel: vi.fn(),
    setIsGenerating: vi.fn(),
    notify: {
      open: vi.fn(),
      success: vi.fn(),
      error: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
      destroy: vi.fn(),
    },
  };
  const tokenStartRef = { current: null as number | null };
  const abortControllerRef = { current: null as AbortController | null };

  const { result } = renderHook(() =>
    useGeneration(
      'resume text',
      'job ad',
      META,
      'ats',
      target,
      'llama',
      m.setStage,
      m.setMeta,
      m.setReport,
      m.setResumeOut,
      m.setCoverOut,
      m.setActiveOut,
      m.setStreamBuffer,
      m.setThinkingBuffer,
      m.setModelLoading,
      m.setTokenCount,
      m.setGenStep,
      m.setError,
      tokenStartRef,
      m.startStageRotation,
      m.stopStageRotation,
      abortControllerRef,
      m.saveAiGeneration,
      (k: string) => k,
      m.setStageLabel,
      m.setIsGenerating,
      m.notify,
      false, // researchCompany
      '', // marketOverride
      [], // emphasis
      provenance?.jobUrl,
      provenance?.board
    )
  );
  return { handleGenerate: result.current.handleGenerate, m, abortControllerRef };
}

const stageCalls = (m: ReturnType<typeof setup>['m']) =>
  m.setStage.mock.calls.map((c) => c[0] as string);

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(generateResume).mockResolvedValue('RESUME');
  vi.mocked(generateCoverLetter).mockResolvedValue({ text: 'COVER', companyBrief: 'BRIEF' });
  vi.mocked(computeQualityReport).mockResolvedValue(null);
});

describe('useGeneration — progressive reveal (#23)', () => {
  it('reveals the résumé (stage done) and finishes both, with a success toast', async () => {
    const { handleGenerate, m } = setup('both');
    await handleGenerate();

    const stages = stageCalls(m);
    expect(stages[0]).toBe('generating');
    // 'done' is set twice: once right after the résumé (progressive reveal) and
    // again at the end — the double-flip is the reveal signature.
    expect(stages.filter((s) => s === 'done').length).toBeGreaterThanOrEqual(2);
    expect(stages.at(-1)).toBe('done');

    expect(m.setIsGenerating).toHaveBeenCalledWith(true);
    expect(m.setIsGenerating).toHaveBeenLastCalledWith(false);
    expect(m.notify.success).toHaveBeenCalledWith({ message: 'aiGenerate.toast.bothReady' });
    // The cover-letter research brief is persisted alongside the documents.
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({
        resumeText: 'RESUME',
        coverLetterText: 'COVER',
        companyBrief: 'BRIEF',
      })
    );
    expect(m.setError).not.toHaveBeenCalledWith(expect.any(String));
  });

  it('keeps the finished résumé when the cover letter fails, and flags it', async () => {
    vi.mocked(generateCoverLetter).mockRejectedValueOnce(new Error('cover boom'));
    const { handleGenerate, m } = setup('both');
    await handleGenerate();

    // The résumé is salvaged: we end on 'done', never bouncing back to configuring.
    expect(stageCalls(m).at(-1)).toBe('done');
    expect(stageCalls(m)).not.toContain('configuring');
    expect(m.notify.error).toHaveBeenCalledWith({ message: 'aiGenerate.toast.coverFailed' });
    // Persisted résumé-only (no cover text / no brief), and no hard error surfaced.
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({ resumeText: 'RESUME', coverLetterText: '', companyBrief: '' })
    );
    expect(m.setError).not.toHaveBeenCalledWith(expect.any(String));
    expect(m.setIsGenerating).toHaveBeenLastCalledWith(false);
  });

  it('surfaces a hard error and returns to configuring when the résumé fails', async () => {
    vi.mocked(generateResume).mockRejectedValueOnce(new Error('resume boom'));
    const { handleGenerate, m } = setup('both');
    await handleGenerate();

    expect(stageCalls(m).at(-1)).toBe('configuring');
    expect(m.setError).toHaveBeenCalledWith('resume boom');
    expect(m.notify.error).toHaveBeenCalledWith({ message: 'aiGenerate.toast.failed' });
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalled();
    expect(m.setIsGenerating).toHaveBeenLastCalledWith(false);
  });
});

describe('useGeneration — single target', () => {
  it('cover-only stays in the streaming view until done, then notifies', async () => {
    const { handleGenerate, m } = setup('cover');
    await handleGenerate();

    const stages = stageCalls(m);
    // No early progressive 'done' for a single document — only the final one.
    expect(stages).toEqual(['generating', 'done']);
    expect(generateResume).not.toHaveBeenCalled();
    expect(m.notify.success).toHaveBeenCalledWith({ message: 'aiGenerate.toast.coverReady' });
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({ resumeText: '', coverLetterText: 'COVER', companyBrief: 'BRIEF' })
    );
  });

  it('resume-only generates just the résumé and notifies', async () => {
    const { handleGenerate, m } = setup('resume');
    await handleGenerate();

    expect(stageCalls(m)).toEqual(['generating', 'done']);
    expect(generateCoverLetter).not.toHaveBeenCalled();
    expect(m.notify.success).toHaveBeenCalledWith({ message: 'aiGenerate.toast.resumeReady' });
  });
});

describe('useGeneration — URL-import provenance (ADR-031)', () => {
  it('persists jobUrl + board when the ad came from a URL import', async () => {
    const { handleGenerate, m } = setup('resume', {
      jobUrl: 'https://boards.greenhouse.io/acme/jobs/1',
      board: 'greenhouse',
    });
    await handleGenerate();

    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://boards.greenhouse.io/acme/jobs/1',
        board: 'greenhouse',
      })
    );
  });

  it('omits jobUrl + board for pasted text (never invents provenance)', async () => {
    const { handleGenerate, m } = setup('resume');
    await handleGenerate();

    expect(m.saveAiGeneration.mutate).toHaveBeenCalled();
    // No call carries provenance keys when the ad wasn't URL-imported.
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ jobUrl: expect.anything() })
    );
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ board: expect.anything() })
    );
  });
});

describe('useGeneration — quality report wiring', () => {
  it('stores the report and carries it on the save when validation succeeds', async () => {
    const report = {
      schemaVersion: 2 as const,
      pipeline: 'fast' as const,
      generatedAt: 1,
      resume: {
        report: {
          ok: true,
          issues: [],
          metrics: {
            keywordCoverage: null,
            topRequirementHits: 0,
            duplicateRatio: 0,
            rolesSource: 0,
            rolesOutput: 0,
          },
        },
        sourceTextHash: 1,
      },
    };
    vi.mocked(computeQualityReport).mockResolvedValueOnce(report);
    const { handleGenerate, m } = setup('resume');
    await handleGenerate();

    expect(m.setReport).toHaveBeenCalledWith(report);
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({ qualityReport: JSON.stringify(report) })
    );
  });

  it('degrades to a report-less save when validation returns null, never blocking the save', async () => {
    vi.mocked(computeQualityReport).mockResolvedValueOnce(null);
    const { handleGenerate, m } = setup('resume');
    await handleGenerate();

    expect(m.setReport).toHaveBeenCalledWith(null);
    expect(m.saveAiGeneration.mutate).toHaveBeenCalled();
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ qualityReport: expect.anything() })
    );
  });

  it('clears the previous report before the progressive-reveal window, so a regenerate in "both" mode never shows the prior run\'s report against the new résumé', async () => {
    let resolveResume!: (v: string) => void;
    vi.mocked(generateResume).mockImplementationOnce(
      () => new Promise((resolve) => (resolveResume = resolve))
    );
    const { handleGenerate, m } = setup('both');

    const run = handleGenerate();
    // handleGenerate clears state — including the report — synchronously,
    // before ever awaiting the résumé generation call.
    await vi.waitFor(() => expect(generateResume).toHaveBeenCalled());
    expect(m.setReport).toHaveBeenCalledWith(null);
    // Still mid-flight: the résumé hasn't resolved, so stage hasn't reached
    // 'done' (the progressive-reveal window) yet — the report was already
    // cleared well before that window opens.
    expect(stageCalls(m)).not.toContain('done');

    resolveResume('RESUME');
    await run;
  });
});

describe('useGeneration — stale-persist guard (Regenerate/reset during validation)', () => {
  const STALE_REPORT = {
    schemaVersion: 2 as const,
    pipeline: 'fast' as const,
    generatedAt: 1,
    resume: {
      report: {
        ok: true,
        issues: [],
        metrics: {
          keywordCoverage: null,
          topRequirementHits: 0,
          duplicateRatio: 0,
          rolesSource: 0,
          rolesOutput: 0,
        },
      },
      sourceTextHash: 1,
    },
  };

  it('a Regenerate click during validation never persists the superseded run', async () => {
    // First call to computeQualityReport (the run about to be superseded) hangs
    // until we resolve it late; every later call resolves immediately.
    let resolveStale!: (r: typeof STALE_REPORT) => void;
    vi.mocked(computeQualityReport).mockImplementationOnce(
      () => new Promise((resolve) => (resolveStale = resolve))
    );
    const { handleGenerate, m } = setup('resume');

    const first = handleGenerate();
    await vi.waitFor(() => expect(computeQualityReport).toHaveBeenCalledTimes(1));

    // Regenerate clicked while the first run is still validating.
    const second = handleGenerate();
    await second;

    // The stale run's validation finally resolves — must be a no-op.
    resolveStale(STALE_REPORT);
    await first;

    // Both runs eagerly clear the report at their own top (2 calls) and the
    // winning (second) run's persist() clears it again once validation
    // resolves null (3rd call) — the superseded run's OWN persist() bails on
    // its aborted-controller check before ever reaching setReport, so no call
    // ever carries the stale run's report. That "never persists twice" is
    // what the save/notify counts below prove.
    expect(m.setReport).toHaveBeenCalledTimes(3);
    expect(m.setReport).toHaveBeenLastCalledWith(null);
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledTimes(1);
    expect(m.notify.success).toHaveBeenCalledTimes(1);
  });

  it('an abort during validation (Reset) skips setReport/save for that run', async () => {
    let resolveStale!: (r: typeof STALE_REPORT) => void;
    vi.mocked(computeQualityReport).mockImplementationOnce(
      () => new Promise((resolve) => (resolveStale = resolve))
    );
    const { handleGenerate, m, abortControllerRef } = setup('resume');

    const run = handleGenerate();
    await vi.waitFor(() => expect(computeQualityReport).toHaveBeenCalledTimes(1));

    // Simulate what AIGeneratePage's reset() now does unconditionally.
    abortControllerRef.current?.abort();

    resolveStale(STALE_REPORT);
    await run;

    // The run's own top-of-function clear still fires once (setReport(null) —
    // never a stale report). Its persist() then bails before EVER calling
    // setReport/mutate again once resolved, and its finally() owns the ref
    // (nothing else replaced it) so it clears it back to null.
    expect(m.setReport).toHaveBeenCalledTimes(1);
    expect(m.setReport).toHaveBeenCalledWith(null);
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalled();
    expect(m.notify.success).not.toHaveBeenCalled();
    expect(abortControllerRef.current).toBeNull();
  });
});

describe('useGeneration — supersede while a stream is in flight (catch race)', () => {
  it('a Regenerate click during the cover-letter stream leaves the newer run untouched when the stale stream later rejects', async () => {
    // Run #1's cover-letter stream hangs until we reject it late; run #2's own
    // cover-letter call also hangs, so run #2 is still mid-flight (not yet
    // finished) when run #1's stale rejection lands.
    let rejectRun1Cover!: (err: Error) => void;
    let resolveRun2Cover!: (r: { text: string; companyBrief: string }) => void;
    vi.mocked(generateCoverLetter)
      .mockImplementationOnce(() => new Promise((_resolve, reject) => (rejectRun1Cover = reject)))
      .mockImplementationOnce(() => new Promise((resolve) => (resolveRun2Cover = resolve)));
    const { handleGenerate, m } = setup('both');

    const first = handleGenerate();
    // Résumé already revealed (progressive reveal); run #1's cover letter is
    // mid-stream.
    await vi.waitFor(() => expect(generateCoverLetter).toHaveBeenCalledTimes(1));

    // Regenerate clicked while run #1's cover letter is still streaming — this
    // aborts run #1's controller and takes over the ref.
    const second = handleGenerate();
    // Run #2's own résumé resolves and it reaches its own cover-letter call,
    // still mid-flight.
    await vi.waitFor(() => expect(generateCoverLetter).toHaveBeenCalledTimes(2));

    const stageCallsBefore = m.setStage.mock.calls.length;
    const stopRotationCallsBefore = m.stopStageRotation.mock.calls.length;
    const streamBufferCallsBefore = m.setStreamBuffer.mock.calls.length;
    const genStepCallsBefore = m.setGenStep.mock.calls.length;

    // Run #1's aborted stream finally rejects — its own supersession, nothing
    // to do with run #2's still-in-flight generation.
    rejectRun1Cover(new Error('cover boom — stale'));
    await first;

    // The superseded run's catch must be a no-op: none of run #2's in-flight
    // UI state gets stomped by run #1's late rejection.
    expect(m.setStage.mock.calls.length).toBe(stageCallsBefore);
    expect(m.stopStageRotation.mock.calls.length).toBe(stopRotationCallsBefore);
    expect(m.setStreamBuffer.mock.calls.length).toBe(streamBufferCallsBefore);
    expect(m.setGenStep.mock.calls.length).toBe(genStepCallsBefore);
    expect(stageCalls(m).at(-1)).toBe('done'); // still run #2's progressive reveal
    expect(m.setIsGenerating).toHaveBeenLastCalledWith(true); // run #2 still in flight

    // Clean up run #2 so no promise is left dangling past the test.
    resolveRun2Cover({ text: 'COVER', companyBrief: 'BRIEF' });
    await second;
  });
});
