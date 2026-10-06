/**
 * Harness for the useScraping suites: stubs the scrape services (the `vi.mock`
 * below applies to every suite that imports this module), resets the module-scoped
 * session store, and wraps the mount / start / watchdog-settle steps they all repeat.
 */
import { expect, type Mock, vi } from 'vitest';
import { act } from '@testing-library/react';

import type { useNotification } from '@ajh/ui';

import { makeJobsDefaults, useSessionStore } from '@/store/session-store';
import { renderHookWithClient } from '@/test-support';

import type { ScrapeFormState } from '../types';
import { useScraping } from './useScraping';

export const mutateAsync: Mock = vi.fn().mockResolvedValue({ jobId: 'j1' });
export const cancelMutateAsync: Mock = vi.fn().mockResolvedValue(undefined);
/** Job-tracker poll used by the watchdog; defaults to a still-running job. */
export const fetchJobMock: Mock = vi.fn().mockResolvedValue({ status: 'running' });
export const invalidatePostingsMock: Mock = vi.fn().mockResolvedValue(undefined);

vi.mock('@/services', async (importActual) => {
  const actual = await importActual<Record<string, unknown>>();
  return {
    ...actual,
    useScrapeBoards: () => ({ mutateAsync }),
    useCancelJob: () => ({ mutateAsync: cancelMutateAsync }),
    fetchJob: (jobId: string) => fetchJobMock(jobId),
    useScrapeProgress: () => null,
    useInvalidatePostings: () => invalidatePostingsMock,
  };
});

export function makeForm(overrides: Partial<ScrapeFormState> = {}): ScrapeFormState {
  return {
    boards: ['linkedin'],
    query: 'engineer',
    location: '',
    radiusKm: 0,
    amount: 25,
    dateFilter: '',
    companies: [],
    workTypes: [],
    ...overrides,
  };
}

const noopNotify = {
  info: vi.fn(),
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
} as unknown as ReturnType<typeof useNotification>;

/** Mount the hook for a fixed `form`. */
export const mount = (form: ScrapeFormState = makeForm()) =>
  renderHookWithClient(() => useScraping(noopNotify, form));

/** Mount the hook so `rerender({ form })` swaps the form. */
export const mountForm = (form: ScrapeFormState) =>
  renderHookWithClient(({ form }: { form: ScrapeFormState }) => useScraping(noopNotify, form), {
    initialProps: { form },
  });

type Mounted = ReturnType<typeof mount>;

/** `startScrape(amount?)` inside `act`. */
export async function start(view: Pick<Mounted, 'result'>, amount?: number) {
  await act(async () => {
    await view.result.current.startScrape(amount);
  });
}

/** Mount and start one scrape. */
export async function mountAndStart(form: ScrapeFormState = makeForm()) {
  const view = mount(form);
  await start(view);
  return view;
}

/** Let the watchdog's first timer-driven poll run (call under `vi.useFakeTimers()`). */
export async function settleWatchdog() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(2600);
  });
}

/** Flush the leading (non-timer) watchdog poll without touching fake timers. */
export async function flushLeadingPoll() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

/** The payload of the nth scrapeBoards call, as the backend received it. */
export const sentPayload = (callIndex: number) =>
  mutateAsync.mock.calls[callIndex]?.[0] as Record<string, unknown> | undefined;

/**
 * The `replace` flag as it reached the BACKEND on the nth scrapeBoards call.
 * Asserting the wire payload (not an internal boolean) is the point: `replace`
 * is what clears the persisted postings cache.
 */
export function sentReplace(callIndex: number): unknown {
  const payload = sentPayload(callIndex);
  expect(payload).toBeDefined();
  return payload?.replace;
}

/** Call from `beforeEach`. */
export function resetScraping() {
  // The session store owns the scrape bookkeeping and is module-scoped, so it
  // leaks between tests unless reset.
  useSessionStore.setState({ jobs: makeJobsDefaults() });
  // Reset the mutation spies HERE, not per test: `sentReplace(n)` indexes into
  // `mutateAsync.mock.calls`, so a test added without a manual clear would
  // silently read a previous test's calls — an order-dependent failure in the
  // very assertions that prove the data-loss fix.
  mutateAsync.mockClear().mockResolvedValue({ jobId: 'j1' });
  cancelMutateAsync.mockClear().mockResolvedValue(undefined);
  fetchJobMock.mockClear().mockResolvedValue({ status: 'running' });
  invalidatePostingsMock.mockClear().mockResolvedValue(undefined);
}
