/**
 * useAutopilotRun — resolved `{ error }` payload handling.
 *
 * The backend `autopilot_run` command RESOLVES (does not reject) with an
 * `{ error }` payload on a scrape failure or unknown id. This exercises the
 * real hook to verify that such a resolution routes to the error state — the
 * SAME state the reject path uses — rather than being treated as 'done'.
 */
import { describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { renderHookWithClient } from '@/test-support';

// ---------------------------------------------------------------------------
// Stubs — declared before the module under test is imported.
// ---------------------------------------------------------------------------

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k, i18n: { language: 'en' } }),
}));

const runMutateAsync = vi.fn();

vi.mock('@/services', async (importActual) => {
  const actual = await importActual<Record<string, unknown>>();
  return {
    ...actual,
    useRunAutopilot: () => ({ mutateAsync: runMutateAsync }),
    usePauseAutopilot: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    useResumeAutopilot: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    useRemoveAutopilot: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    useAutopilotStepEvents: () => {},
  };
});

// Import under test AFTER mocks.
import { useAutopilotRun } from './useAutopilotRun';

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('useAutopilotRun — handleRun error-payload handling', () => {
  it('routes a resolved { error } payload to the error state, not done', async () => {
    runMutateAsync.mockResolvedValueOnce({ error: 'scrape failed: 429', jobId: 'j1' });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap1');
    });

    expect(result.current.runStates.ap1).toBe('error');
    expect(result.current.error).toBe('scrape failed: 429');
  });

  it('routes a resolved success payload to the done state', async () => {
    runMutateAsync.mockResolvedValueOnce({ jobId: 'j2', found: 3, applied: 0 });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap2');
    });

    expect(result.current.runStates.ap2).toBe('done');
    expect(result.current.error).toBeNull();
  });
});
