import { createElement, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import { useGenerationStore } from '@/store/generation-store';

import { useTailorGeneration } from './useTailorGeneration';

// i18n: identity translator so phaseLabel resolves without the i18n runtime.
vi.mock('@/lib/i18n', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

// Capture the saved application record so we can assert the job link is attached.
const save = vi.fn().mockResolvedValue({ id: 'gen-1', success: true });
vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({ aiGenerations: { save } }),
}));

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
  jobUrl: 'https://acme.com/job/42',
  board: 'linkedin',
  researchCompany: false,
};

// useTailorGeneration uses useQueryClient (to invalidate after save), so the hook
// must render under a QueryClientProvider.
const wrapper = ({ children }: { children: ReactNode }) =>
  createElement(QueryClientProvider, { client: new QueryClient() }, children);
const render = (p = params) => renderHook(() => useTailorGeneration(p), { wrapper });

describe('useTailorGeneration', () => {
  // The session lives in the shared store — reset it so tests don't leak state.
  beforeEach(() => {
    useGenerationStore.setState({ sessions: {} });
    save.mockClear();
  });

  it('starts idle with empty buffers', () => {
    const { result } = render();
    expect(result.current.phase).toBe('idle');
    expect(result.current.resumeOut).toBe('');
    expect(result.current.coverOut).toBe('');
    expect(result.current.thinking).toBe('');
  });

  it('threads reasoning into `thinking`, clearing it between phases', async () => {
    const { result } = render();

    await act(async () => {
      await result.current.generate('my resume', 'both');
    });

    expect(result.current.resumeOut).toBe('RESUME');
    expect(result.current.coverOut).toBe('COVER');
    // Cleared at the cover phase, so only the cover-letter reasoning remains.
    expect(result.current.thinking).toBe('C-think');
    expect(result.current.phase).toBe('idle');
  });

  it('saves the application linked to the job url + board after a clean run', async () => {
    const { result } = render();

    await act(async () => {
      await result.current.generate('my resume', 'both');
    });

    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://acme.com/job/42',
        board: 'linkedin',
        jobAd: 'JD',
        resumeText: 'RESUME',
        coverLetterText: 'COVER',
        mode: 'ats',
      })
    );
  });

  it('does not generate or save when AI is unavailable', async () => {
    const { result } = render({ ...params, canUse: false });
    await act(async () => {
      await result.current.generate('my resume', 'resume');
    });
    expect(result.current.resumeOut).toBe('');
    expect(result.current.generating).toBe(false);
    expect(save).not.toHaveBeenCalled();
  });
});
