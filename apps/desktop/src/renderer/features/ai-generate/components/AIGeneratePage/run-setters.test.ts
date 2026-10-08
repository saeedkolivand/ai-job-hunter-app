/** Rule 16: AI Generate's live run state outlives the page, without touching the session store per token. */
import { beforeEach, describe, expect, it } from 'vitest';

import {
  resetAIGenerateAll,
  runAbortRef,
  useAIGenerateRunStore,
  useSessionStore,
} from '@/store/session-store';

import { runSetters } from './run-setters';

beforeEach(resetAIGenerateAll);

describe('runSetters', () => {
  it('streams per-token text into the run store only; a plain value commits to the session', () => {
    const before = useSessionStore.getState().aiGenerate;
    runSetters.setResumeOut((p) => p + 'x');
    runSetters.setResumeOut((p) => p + 'y');
    expect(useAIGenerateRunStore.getState().liveResume).toBe('xy');
    expect(useSessionStore.getState().aiGenerate).toBe(before);

    runSetters.setResumeOut('final');
    expect(useAIGenerateRunStore.getState().liveResume).toBe('');
    expect(useSessionStore.getState().aiGenerate.resumeOut).toBe('final');
  });
});

describe('resetAIGenerateAll', () => {
  it('aborts the live run, clears run + session state, and disowns the controller', () => {
    const c = new AbortController();
    runAbortRef.current = c;
    runSetters.setIsGenerating(true);
    useSessionStore.getState().setAIGenerate({ resume: 'keep?' });

    resetAIGenerateAll();

    expect(c.signal.aborted).toBe(true);
    expect(runAbortRef.current).toBeNull();
    expect(useAIGenerateRunStore.getState().isGenerating).toBe(false);
    expect(useSessionStore.getState().aiGenerate.resume).toBe('');
  });
});
