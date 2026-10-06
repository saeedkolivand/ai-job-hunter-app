import { beforeEach, type Mock, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import {
  computeQualityReport,
  generateCoverLetter,
  generateResume,
  type GenerationMeta,
} from '@/lib/generate';

import { useGeneration } from './useGeneration';

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

export type Target = 'resume' | 'cover' | 'both';

/** The vi.fn() setters `setup` threads into `useGeneration`, for assertions. */
export interface Setters {
  setStage: Mock;
  setMeta: Mock;
  setReport: Mock;
  setResumeOut: Mock;
  setCoverOut: Mock;
  setActiveOut: Mock;
  setStreamBuffer: Mock;
  setThinkingBuffer: Mock;
  setModelLoading: Mock;
  setTokenCount: Mock;
  setGenStep: Mock;
  setError: Mock;
  startStageRotation: Mock;
  stopStageRotation: Mock;
  saveAiGeneration: { mutate: Mock };
  setStageLabel: Mock;
  setIsGenerating: Mock;
  notify: Record<'open' | 'success' | 'error' | 'info' | 'warning' | 'destroy', Mock>;
}

/**
 * Build the (large, positional) useGeneration arg list with vi.fn() setters.
 * `useGeneration` is a plain factory (no React hooks inside), but it is named
 * like a hook, so it is invoked via `renderHook` to satisfy rules-of-hooks.
 */
export function setup(
  target: Target,
  provenance?: { jobUrl?: string; board?: string }
): {
  handleGenerate: ReturnType<typeof useGeneration>['handleGenerate'];
  m: Setters;
  abortControllerRef: { current: AbortController | null };
} {
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

export async function runGeneration(
  target: Target,
  provenance?: { jobUrl?: string; board?: string }
): Promise<ReturnType<typeof setup>> {
  const harness = setup(target, provenance);
  await harness.handleGenerate();
  return harness;
}

export const stageCalls = (m: Setters) => m.setStage.mock.calls.map((c) => c[0] as string);

/** Reset the generate mocks before every test; call once at module top level. */
export function installGenerationMocks() {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(generateResume).mockResolvedValue('RESUME');
    vi.mocked(generateCoverLetter).mockResolvedValue({ text: 'COVER', companyBrief: 'BRIEF' });
    vi.mocked(computeQualityReport).mockResolvedValue(null);
  });
}
