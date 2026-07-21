import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { generateCoverLetter, generateResume, type GenerationMeta } from '@/lib/generate';

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
  return { handleGenerate: result.current.handleGenerate, m };
}

const stageCalls = (m: ReturnType<typeof setup>['m']) =>
  m.setStage.mock.calls.map((c) => c[0] as string);

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(generateResume).mockResolvedValue('RESUME');
  vi.mocked(generateCoverLetter).mockResolvedValue({ text: 'COVER', companyBrief: 'BRIEF' });
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
