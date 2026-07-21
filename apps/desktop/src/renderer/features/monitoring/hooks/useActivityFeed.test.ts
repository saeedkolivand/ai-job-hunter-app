/**
 * useActivityFeed — live/historical merge.
 *
 * The feed shows live job events on top and the persisted job records below,
 * deduped so a job that just finished is not listed twice. The dedup keys the
 * live item's id back to the job id it was built from.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

// ── services ─────────────────────────────────────────────────────────────────

const mockFetchJob = vi.fn();
/** The job-event handler the hook registers, so a test can deliver events. */
const jobEvents = vi.hoisted(() => ({
  handler: null as ((event: unknown) => void) | null,
}));

vi.mock('@/services', () => ({
  fetchJob: (...args: unknown[]) => mockFetchJob(...args),
  useJobEvents: (cb: (event: unknown) => void) => {
    jobEvents.handler = cb;
  },
}));

import type { JobRecord } from '@/features/monitoring/types';

import { useActivityFeed } from './useActivityFeed';

// ── fixtures ─────────────────────────────────────────────────────────────────

/** Real shape: `format!("job-{}", Uuid::new_v4())` on the Rust side. */
const JOB_ID = 'job-3f2504e0-4f89-11d3-9a0c-0305e82c3301';

function completedJob(overrides: Partial<JobRecord> = {}): JobRecord {
  return {
    id: JOB_ID,
    kind: 'scrape.run',
    status: 'completed',
    progress: 1,
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_001_000,
    finishedAt: 1_700_000_001_000,
    ...overrides,
  };
}

beforeEach(() => {
  jobEvents.handler = null;
  mockFetchJob.mockReset();
  mockFetchJob.mockResolvedValue(completedJob());
});

// ── tests ────────────────────────────────────────────────────────────────────

describe('useActivityFeed — live/historical dedup', () => {
  it('lists a just-completed job once, not twice', async () => {
    const { result } = renderHook(() =>
      useActivityFeed([completedJob()], { 'scrape.run': 'Scrape' })
    );

    // Only the historical row so far.
    expect(result.current.activity).toHaveLength(1);

    // The same job completes live. Its live id is `${jobId}-${ts}`, so the
    // historical row for that job must now be filtered out — not rendered again.
    await act(async () => {
      jobEvents.handler?.({ type: 'job.completed', jobId: JOB_ID, ts: 1_700_000_001_500 });
    });

    await waitFor(() => expect(result.current.liveActivity).toHaveLength(1));
    expect(result.current.activity).toHaveLength(1);
    expect(result.current.activity[0]?.id).toBe(`${JOB_ID}-1700000001500`);
  });

  it('keeps a historical job that has no live counterpart', async () => {
    const other = completedJob({ id: 'job-11111111-2222-3333-4444-555555555555' });
    const { result } = renderHook(() =>
      useActivityFeed([completedJob(), other], { 'scrape.run': 'Scrape' })
    );

    await act(async () => {
      jobEvents.handler?.({ type: 'job.completed', jobId: JOB_ID, ts: 1_700_000_001_500 });
    });

    await waitFor(() => expect(result.current.liveActivity).toHaveLength(1));
    // One live row for JOB_ID + the untouched historical row for the other job.
    expect(result.current.activity).toHaveLength(2);
    expect(result.current.activity.map((a) => a.id)).toEqual([`${JOB_ID}-1700000001500`, other.id]);
  });
});
