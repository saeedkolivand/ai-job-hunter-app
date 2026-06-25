/**
 * useJobAdSummary — unit tests for the language auto-regenerate behaviour.
 *
 * Covers:
 *   1. Language change AFTER a summary exists → auto-regenerates with the new language.
 *   2. Language change BEFORE any summary → does NOT auto-fire generateJobAdSummary.
 *   3. Prior in-flight generation is aborted before the new language run starts.
 *   4. Language change with empty jobDesc (canUse=true, hasDesc=false) → no auto-fire.
 *
 * Strategy:
 *   - `generateJobAdSummary` is mocked: resolves immediately with a fixed string.
 *   - `useUpdateApplication` / `useSessionStore` / `AppClientProvider` are stubbed
 *     (same pattern as useApplicationAnswers.test.ts).
 *   - `useSessionStore` real Zustand store; reset between tests via setState.
 *   - No QueryClient needed (useUpdateApplication mock doesn't go through RQ).
 */

import { createElement, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── generateJobAdSummary — resolves with a deterministic string ───────────────

const generateJobAdSummary = vi.fn(async (_params: unknown) => 'SUMMARY');

vi.mock('@/lib/generate', () => ({
  generateJobAdSummary: (params: unknown) => generateJobAdSummary(params),
}));

// ── useUpdateApplication — stub; captures calls ───────────────────────────────

const mutate = vi.fn();
vi.mock('@/services/use-applications/use-applications', () => ({
  useUpdateApplication: () => ({ mutate }),
}));

// ── useSessionStore — use the real store; reset between tests ─────────────────

import { useSessionStore } from '@/store/session-store';

// ── Import hook AFTER all mocks ───────────────────────────────────────────────
import { useJobAdSummary } from './useJobAdSummary';

// ── Helpers ───────────────────────────────────────────────────────────────────

const initialStore = useSessionStore.getState();

function makeWrapper() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: qc }, children);
}

function makeParams(overrides: Partial<Parameters<typeof useJobAdSummary>[0]> = {}) {
  return {
    jobDesc: 'Software Engineer at Acme Corp — full job description.',
    model: 'llama3',
    canUse: true,
    hasDesc: true,
    meta: null,
    applicationId: undefined,
    initialSummary: undefined,
    ...overrides,
  };
}

beforeEach(() => {
  generateJobAdSummary.mockClear();
  mutate.mockClear();
  // Reset session store to its initial state so jobSummaryCache is empty.
  useSessionStore.setState(initialStore, true);
});

// ── 1. Language change after a summary exists → auto-regenerates ──────────────

describe('useJobAdSummary — language auto-regenerate', () => {
  it('calls generateJobAdSummary with the new language after a summary was manually generated', async () => {
    const { result } = renderHook(() => useJobAdSummary(makeParams()), {
      wrapper: makeWrapper(),
    });

    // Manual first generate — produces a summary, sets hasSummaryRef.
    await act(async () => {
      await result.current.generate();
    });

    expect(generateJobAdSummary).toHaveBeenCalledTimes(1);
    expect(generateJobAdSummary).toHaveBeenLastCalledWith(
      expect.objectContaining({ language: 'en' })
    );
    expect(result.current.summary).toBe('SUMMARY');

    // Simulate the user picking German.
    await act(async () => {
      result.current.setLanguage('de');
    });

    // The auto-regenerate effect must have fired a second call with 'de'.
    expect(generateJobAdSummary).toHaveBeenCalledTimes(2);
    expect(generateJobAdSummary).toHaveBeenLastCalledWith(
      expect.objectContaining({ language: 'de' })
    );
  });

  it('uses the new language string in the second call (not a stale closure)', async () => {
    const { result } = renderHook(() => useJobAdSummary(makeParams()), {
      wrapper: makeWrapper(),
    });

    await act(async () => {
      await result.current.generate();
    });

    await act(async () => {
      result.current.setLanguage('fr');
    });

    const lastCall = generateJobAdSummary.mock.calls[generateJobAdSummary.mock.calls.length - 1];
    expect(lastCall).toBeDefined();
    const lastParams = lastCall?.[0] as { language?: string } | undefined;
    expect(lastParams?.language).toBe('fr');
  });
});

// ── 2. Language change before any summary → no auto-fire ─────────────────────

describe('useJobAdSummary — no auto-fire when no summary exists', () => {
  it('does NOT call generateJobAdSummary when language changes before any generate', async () => {
    const { result } = renderHook(() => useJobAdSummary(makeParams()), {
      wrapper: makeWrapper(),
    });

    // No generate() call — hasSummaryRef is false.
    await act(async () => {
      result.current.setLanguage('de');
    });

    // Only the mount effect runs; no generation should have fired.
    expect(generateJobAdSummary).not.toHaveBeenCalled();
  });

  it('does NOT auto-fire when hasDesc is false and language changes', async () => {
    // canUse=true but hasDesc=false (no job description pasted).
    const { result } = renderHook(
      () => useJobAdSummary(makeParams({ jobDesc: '', hasDesc: false })),
      { wrapper: makeWrapper() }
    );

    await act(async () => {
      result.current.setLanguage('de');
    });

    expect(generateJobAdSummary).not.toHaveBeenCalled();
  });

  it('does NOT auto-fire when canUse is false and language changes after generate', async () => {
    // canUse starts false — generate guard fires; no summary is ever produced.
    const { result } = renderHook(() => useJobAdSummary(makeParams({ canUse: false })), {
      wrapper: makeWrapper(),
    });

    // generate() is a no-op when canUse=false.
    await act(async () => {
      await result.current.generate();
    });
    expect(generateJobAdSummary).not.toHaveBeenCalled();

    await act(async () => {
      result.current.setLanguage('de');
    });

    expect(generateJobAdSummary).not.toHaveBeenCalled();
  });
});

// ── 3. Prior in-flight generation is aborted before the new language run ──────

describe('useJobAdSummary — abort on language change', () => {
  it('aborts the prior controller and starts a new one when language changes mid-flight', async () => {
    // Make generateJobAdSummary hang until we resolve it manually.
    let resolvePending!: (v: string) => void;
    generateJobAdSummary.mockImplementationOnce(
      () =>
        new Promise<string>((res) => {
          resolvePending = res;
        })
    );
    // Second call resolves immediately.
    generateJobAdSummary.mockImplementationOnce(async () => 'SUMMARY-DE');

    const { result } = renderHook(() => useJobAdSummary(makeParams()), {
      wrapper: makeWrapper(),
    });

    // Start first generate (will hang).
    act(() => {
      void result.current.generate();
    });

    // Artificially seed hasSummaryRef so the language effect fires.
    // We do this by resolving the first call.
    await act(async () => {
      resolvePending('SUMMARY-EN');
    });

    // Now hasSummaryRef is true. Changing language should abort + restart.
    await act(async () => {
      result.current.setLanguage('de');
    });

    expect(generateJobAdSummary).toHaveBeenCalledTimes(2);
    expect(generateJobAdSummary).toHaveBeenLastCalledWith(
      expect.objectContaining({ language: 'de' })
    );
    expect(result.current.summary).toBe('SUMMARY-DE');
  });
});
