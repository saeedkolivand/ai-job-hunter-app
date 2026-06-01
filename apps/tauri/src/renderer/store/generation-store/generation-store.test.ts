import { beforeEach, describe, expect, it, vi } from 'vitest';

import { generateResume } from '@/lib/generate';

import { EMPTY_SESSION, useGenerationStore } from './generation-store';

// Stub the generation pipeline; resume/cover emit one reasoning + one content token.
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
      _r: string,
      _j: string,
      _m: unknown,
      _mode: string,
      _model: string,
      onToken: (t: string) => void,
      _l?: string,
      _s?: AbortSignal,
      onThinking?: (t: string) => void
    ) => {
      onThinking?.('R-think');
      onToken('RESUME');
      return 'RESUME';
    }
  ),
  generateCoverLetter: vi.fn(
    async (
      _r: string,
      _j: string,
      _m: unknown,
      _mode: string,
      _model: string,
      onToken: (t: string) => void,
      _l?: string,
      _s?: AbortSignal,
      onThinking?: (t: string) => void
    ) => {
      onThinking?.('C-think');
      onToken('COVER');
      return 'COVER';
    }
  ),
}));

const t = (k: string) => k;
const base = { resume: 'r', jobDesc: 'jd', model: 'llama', mode: 'ats' as const, t };

beforeEach(() => useGenerationStore.setState({ sessions: {} }));

describe('generation store', () => {
  it('returns one stable empty session reference for unknown ids', () => {
    const s = useGenerationStore.getState();
    expect(s.getSession('nope')).toBe(EMPTY_SESSION);
    expect(s.getSession('other')).toBe(EMPTY_SESSION);
  });

  it('runs analyze → resume → cover into the session, surviving a remount', async () => {
    const id = 'autopilot:job-1';
    await useGenerationStore.getState().runTailor({ contextId: id, target: 'both', ...base });

    // A fresh getState read simulates a component remounting after navigation:
    // the session is still there, fully populated.
    const s = useGenerationStore.getState().getSession(id);
    expect(s.resumeOut).toBe('RESUME');
    expect(s.coverOut).toBe('COVER');
    expect(s.thinking).toBe('C-think'); // reasoning cleared at the cover phase
    expect(s.generating).toBe(false);
    expect(s.phase).toBe('idle');
    expect(s.meta).not.toBeNull();
  });

  it('keeps sessions isolated per context id', async () => {
    await useGenerationStore.getState().runTailor({ contextId: 'a', target: 'resume', ...base });
    expect(useGenerationStore.getState().getSession('a').resumeOut).toBe('RESUME');
    expect(useGenerationStore.getState().getSession('b')).toBe(EMPTY_SESSION);
  });

  it('ignores a concurrent run for a context that is already generating', async () => {
    const id = 'busy';
    useGenerationStore.setState({ sessions: { [id]: { ...EMPTY_SESSION, generating: true } } });
    await useGenerationStore.getState().runTailor({ contextId: id, target: 'resume', ...base });
    expect(useGenerationStore.getState().getSession(id).resumeOut).toBe(''); // guard returned early
  });

  it('reset drops a session', () => {
    const id = 'done';
    useGenerationStore.setState({ sessions: { [id]: { ...EMPTY_SESSION, resumeOut: 'X' } } });
    useGenerationStore.getState().reset(id);
    expect(useGenerationStore.getState().getSession(id)).toBe(EMPTY_SESSION);
  });

  it('calls onComplete with the finished documents + metadata after a clean run', async () => {
    const onComplete = vi.fn();
    await useGenerationStore
      .getState()
      .runTailor({ contextId: 'c1', target: 'both', onComplete, ...base });

    expect(onComplete).toHaveBeenCalledTimes(1);
    expect(onComplete).toHaveBeenCalledWith({
      meta: expect.objectContaining({ resumeLanguage: 'en' }),
      resumeText: 'RESUME',
      coverLetterText: 'COVER',
    });
  });

  it('does not call onComplete when the run fails (so a failed application is never saved)', async () => {
    vi.mocked(generateResume).mockRejectedValueOnce(new Error('boom'));
    const onComplete = vi.fn();
    await useGenerationStore
      .getState()
      .runTailor({ contextId: 'c2', target: 'resume', onComplete, ...base });

    expect(onComplete).not.toHaveBeenCalled();
    expect(useGenerationStore.getState().getSession('c2').error).toBe('boom');
  });
});
