import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useGenerationStore } from '@/store/generation-store';

import { useTailorGeneration } from './useTailorGeneration';

// i18n: identity translator so phaseLabel resolves without the i18n runtime.
vi.mock('@/lib/i18n', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

// Stub the generation pipeline. Resume/cover stubs emit a reasoning token via
// the `onThinking` callback (last arg) so we can assert it threads through.
vi.mock('@/lib/generate', () => ({
  extractMetadata: vi.fn().mockResolvedValue({
    candidateName: '',
    jobTitle: '',
    companyName: '',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
    topRequirements: [],
  }),
  generateResume: vi.fn(
    async (
      _resume: string,
      _jobAd: string,
      _meta: unknown,
      _mode: string,
      _model: string,
      onToken: (t: string) => void,
      _locale?: string,
      _signal?: AbortSignal,
      onThinking?: (t: string) => void
    ) => {
      onThinking?.('R-think');
      onToken('R');
      return 'RESUME';
    }
  ),
  generateCoverLetter: vi.fn(
    async (
      _resume: string,
      _jobAd: string,
      _meta: unknown,
      _mode: string,
      _model: string,
      onToken: (t: string) => void,
      _locale?: string,
      _signal?: AbortSignal,
      onThinking?: (t: string) => void
    ) => {
      onThinking?.('C-think');
      onToken('C');
      return 'COVER';
    }
  ),
  buildFilename: vi.fn(),
  exportDOCX: vi.fn(),
  exportPDF: vi.fn(),
  exportTXT: vi.fn(),
}));

const params = {
  contextId: 'autopilot:test-job',
  jobDesc: 'JD',
  model: 'llama',
  canUse: true,
  hasDesc: true,
};

describe('useTailorGeneration', () => {
  // The session lives in the shared store — reset it so tests don't leak state.
  beforeEach(() => useGenerationStore.setState({ sessions: {} }));

  it('starts idle with empty buffers', () => {
    const { result } = renderHook(() => useTailorGeneration(params));
    expect(result.current.phase).toBe('idle');
    expect(result.current.resumeOut).toBe('');
    expect(result.current.coverOut).toBe('');
    expect(result.current.thinking).toBe('');
  });

  it('threads reasoning into `thinking`, clearing it between phases', async () => {
    const { result } = renderHook(() => useTailorGeneration(params));

    await act(async () => {
      await result.current.generate('my resume', 'both');
    });

    expect(result.current.resumeOut).toBe('RESUME');
    expect(result.current.coverOut).toBe('COVER');
    // Cleared at the cover phase, so only the cover-letter reasoning remains.
    expect(result.current.thinking).toBe('C-think');
    expect(result.current.phase).toBe('idle');
  });

  it('does not generate when AI is unavailable', async () => {
    const { result } = renderHook(() => useTailorGeneration({ ...params, canUse: false }));
    await act(async () => {
      await result.current.generate('my resume', 'resume');
    });
    expect(result.current.resumeOut).toBe('');
    expect(result.current.generating).toBe(false);
  });
});
