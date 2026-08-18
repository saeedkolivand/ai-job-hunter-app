/**
 * useJobEvents — invalidation coverage (LOW #2, board-health chip staleness).
 *
 * `keys.boards.health` has `staleTime: 10s` and no `refetchInterval`, so
 * nothing refreshes an already-mounted AutopilotCard's reliability badge when
 * a scrape/autopilot run finishes — only window focus/remount did, which
 * meant the chip could show the PREVIOUS verdict right after a run completed
 * on-screen. `useJobEvents` is the always-mounted subscription (via
 * `useWorkerActivity` → `StatusBar`, see scripts/check-event-subscriptions.mjs)
 * that already invalidates `keys.jobs.all` on every event; this asserts it
 * also invalidates `keys.boards.health`.
 */
import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { keys } from '../query-client';

const invalidateQueries = vi.fn();
// A stable no-op default (not `| undefined`) so the test never has to narrow
// an optional across the `renderHook` call boundary — the mock below
// overwrites it with the real handler as soon as `useJobEvents` subscribes.
let registeredHandler: (event: unknown) => void = () => {};

vi.mock('@tanstack/react-query', async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  return {
    ...actual,
    useQueryClient: () => ({ invalidateQueries }),
  };
});

vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({
    jobs: {
      onEvent: (handler: (event: unknown) => void) => {
        registeredHandler = handler;
        return () => {};
      },
    },
  }),
}));

import { useJobEvents } from './use-jobs';

describe('useJobEvents', () => {
  it('invalidates keys.boards.health (not just keys.jobs.all) on every job event', () => {
    invalidateQueries.mockClear();
    renderHook(() => useJobEvents());

    registeredHandler({ type: 'job.completed', jobId: 'job-1', ts: Date.now() });

    const keysInvalidated = invalidateQueries.mock.calls.map((call) => call[0]?.queryKey);
    expect(keysInvalidated).toContainEqual(keys.jobs.all);
    expect(keysInvalidated).toContainEqual(keys.boards.health);
  });
});
