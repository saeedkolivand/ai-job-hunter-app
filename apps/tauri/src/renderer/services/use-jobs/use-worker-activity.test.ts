/**
 * useWorkerActivity — unit tests.
 *
 * useWorkerActivity lives in the same module as useJobQueue / useJobEvents.
 * We cannot partially mock a module and still import useWorkerActivity from it,
 * so we mock its two collaborators at the provider layer instead:
 *   - @tanstack/react-query  → stubbed useQuery that returns controllable data
 *   - AppClientProvider      → stubbed useAppClient so useJobEvents can subscribe
 *
 * This keeps the real useWorkerActivity logic under test while eliminating
 * all IPC / QueryClient setup.
 *
 * Covers:
 *  - active count = number of running + streaming jobs.
 *  - queued count = number of queued (pending) jobs.
 *  - isActive is true when active > 0, false otherwise.
 *  - byKind groups running jobs by kind and applies kindLabelMap labels.
 *  - Unknown kind falls back to the raw kind string.
 *  - Empty job list → zeros, empty arrays, isActive false.
 */
import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import type { JobKind, JobRecord } from '@ajh/shared';

// ── stub React Query ──────────────────────────────────────────────────────────
// useJobQueue calls useQuery; we replace useQuery with a controllable stub.
// useQueryClient is used by useJobEvents' invalidation — return a no-op stub.

let stubbedJobs: JobRecord[] = [];

vi.mock('@tanstack/react-query', async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  return {
    ...actual,
    useQuery: () => ({ data: stubbedJobs, isLoading: false, isError: false }),
    useQueryClient: () => ({ invalidateQueries: vi.fn() }),
  };
});

// ── stub AppClientProvider ────────────────────────────────────────────────────
// useJobEvents calls useAppClient().jobs.onEvent — return a no-op unsubscribe.

vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({
    jobs: {
      onEvent: vi.fn(() => () => {}),
    },
  }),
}));

import { useWorkerActivity } from './use-jobs';

// ── fixtures ──────────────────────────────────────────────────────────────────

function makeJob(id: string, kind: JobKind, status: JobRecord['status']): JobRecord {
  return {
    id,
    kind,
    status,
    progress: 0,
    payload: null,
    retries: 0,
    maxRetries: 3,
    createdAt: Date.now(),
    updatedAt: Date.now(),
  };
}

const KIND_MAP: Record<string, string> = {
  'ai.generate': 'AI Generation',
  'scrape.board': 'Board Scrape',
};

// ── tests ─────────────────────────────────────────────────────────────────────

describe('useWorkerActivity — empty queue', () => {
  it('returns zeros and empty arrays when there are no jobs', () => {
    stubbedJobs = [];
    const { result } = renderHook(() => useWorkerActivity(KIND_MAP));
    expect(result.current.active).toBe(0);
    expect(result.current.queued).toBe(0);
    expect(result.current.isActive).toBe(false);
    expect(result.current.byKind).toEqual([]);
    expect(result.current.running).toEqual([]);
    expect(result.current.queuedJobs).toEqual([]);
  });
});

describe('useWorkerActivity — mixed job statuses', () => {
  it('counts running and streaming as active, queued as queued', () => {
    stubbedJobs = [
      makeJob('j1', 'ai.generate', 'running'),
      makeJob('j2', 'ai.generate', 'streaming'),
      makeJob('j3', 'scrape.board', 'queued'),
      makeJob('j4', 'document.import', 'completed'),
    ];
    const { result } = renderHook(() => useWorkerActivity(KIND_MAP));
    expect(result.current.active).toBe(2);
    expect(result.current.queued).toBe(1);
    expect(result.current.isActive).toBe(true);
  });

  it('populates running and queuedJobs arrays correctly', () => {
    const j1 = makeJob('j1', 'ai.generate', 'running');
    const j2 = makeJob('j2', 'scrape.board', 'queued');
    const j3 = makeJob('j3', 'document.import', 'completed');
    stubbedJobs = [j1, j2, j3];
    const { result } = renderHook(() => useWorkerActivity(KIND_MAP));
    expect(result.current.running).toEqual([j1]);
    expect(result.current.queuedJobs).toEqual([j2]);
  });
});

describe('useWorkerActivity — byKind grouping', () => {
  it('groups running jobs by kind and applies kindLabelMap labels', () => {
    stubbedJobs = [
      makeJob('j1', 'ai.generate', 'running'),
      makeJob('j2', 'ai.generate', 'streaming'),
      makeJob('j3', 'scrape.board', 'running'),
    ];
    const { result } = renderHook(() => useWorkerActivity(KIND_MAP));
    const { byKind } = result.current;

    expect(byKind).toHaveLength(2);

    const aiEntry = byKind.find((e) => e.kind === 'ai.generate');
    expect(aiEntry).toBeDefined();
    expect(aiEntry?.label).toBe('AI Generation');
    expect(aiEntry?.count).toBe(2);

    const scrapeEntry = byKind.find((e) => e.kind === 'scrape.board');
    expect(scrapeEntry).toBeDefined();
    expect(scrapeEntry?.label).toBe('Board Scrape');
    expect(scrapeEntry?.count).toBe(1);
  });

  it('falls back to the raw kind string when no label exists in the map', () => {
    stubbedJobs = [makeJob('j1', 'document.import', 'running')];
    const { result } = renderHook(() => useWorkerActivity(KIND_MAP));
    const entry = result.current.byKind.find((e) => e.kind === 'document.import');
    expect(entry?.label).toBe('document.import');
  });

  it('does not include queued or completed jobs in byKind', () => {
    stubbedJobs = [
      makeJob('j1', 'ai.generate', 'queued'),
      makeJob('j2', 'scrape.board', 'completed'),
    ];
    const { result } = renderHook(() => useWorkerActivity(KIND_MAP));
    expect(result.current.byKind).toHaveLength(0);
  });
});

describe('useWorkerActivity — isActive flag', () => {
  it('is false when all jobs are completed or failed', () => {
    stubbedJobs = [
      makeJob('j1', 'ai.generate', 'completed'),
      makeJob('j2', 'scrape.board', 'failed'),
    ];
    const { result } = renderHook(() => useWorkerActivity(KIND_MAP));
    expect(result.current.isActive).toBe(false);
    expect(result.current.active).toBe(0);
  });

  it('is true when at least one job is streaming', () => {
    stubbedJobs = [makeJob('j1', 'ai.generate', 'streaming')];
    const { result } = renderHook(() => useWorkerActivity(KIND_MAP));
    expect(result.current.isActive).toBe(true);
  });
});
