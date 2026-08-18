/**
 * useScraping — companies payload guard.
 *
 * Exercises the actual hook (not a local clone) to verify that the
 * `companies` field is conditionally included in the scrapeBoards payload
 * so the IPC contract is honoured and the Rust engine's is_empty() skip
 * check behaves correctly.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import type { useNotification } from '@ajh/ui';

import { makeJobsDefaults, useSessionStore } from '@/store/session-store';
import { renderHookWithClient } from '@/test-support';

// ---------------------------------------------------------------------------
// Stubs — must be declared before imports that trigger the module under test.
// ---------------------------------------------------------------------------

const mutateAsync = vi.fn().mockResolvedValue({ jobId: 'j1' });
const cancelMutateAsync = vi.fn().mockResolvedValue(undefined);
/** Job-tracker poll used by the watchdog; defaults to a still-running job. */
const fetchJobMock = vi.fn().mockResolvedValue({ status: 'running' });

vi.mock('@/services', async (importActual) => {
  const actual = await importActual<Record<string, unknown>>();
  return {
    ...actual,
    useScrapeBoards: () => ({ mutateAsync }),
    useCancelJob: () => ({ mutateAsync: cancelMutateAsync }),
    fetchJob: (jobId: string) => fetchJobMock(jobId),
    useScrapeProgress: () => null,
    useInvalidatePostings: () => vi.fn().mockResolvedValue(undefined),
  };
});

// Import under test AFTER mocks.
import type { ScrapeFormState } from '../types';
import { useScraping } from './useScraping';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeForm(overrides: Partial<ScrapeFormState> = {}): ScrapeFormState {
  return {
    boards: ['linkedin'],
    query: 'engineer',
    location: '',
    radiusKm: 0,
    amount: 25,
    dateFilter: '',
    companies: [],
    ...overrides,
  };
}

const noopNotify = {
  info: vi.fn(),
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
} as unknown as ReturnType<typeof useNotification>;

/**
 * The `replace` flag as it reached the BACKEND on the nth scrapeBoards call.
 * Asserting the wire payload (not an internal boolean) is the point: `replace`
 * is what clears the persisted postings cache.
 */
function sentReplace(callIndex: number): unknown {
  const payload = mutateAsync.mock.calls[callIndex]?.[0] as Record<string, unknown> | undefined;
  expect(payload).toBeDefined();
  return payload?.replace;
}

beforeEach(() => {
  // The session store now owns the scrape bookkeeping and is module-scoped, so
  // it leaks between tests unless reset.
  useSessionStore.setState({ jobs: makeJobsDefaults() });
  // Reset the mutation spies HERE, not per test: `sentReplace(n)` indexes into
  // `mutateAsync.mock.calls`, so a test added without a manual clear would
  // silently read a previous test's calls — an order-dependent failure in the
  // very assertions that prove the data-loss fix.
  mutateAsync.mockClear().mockResolvedValue({ jobId: 'j1' });
  cancelMutateAsync.mockClear().mockResolvedValue(undefined);
  fetchJobMock.mockClear().mockResolvedValue({ status: 'running' });
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('useScraping — companies field in scrapeBoards payload', () => {
  it('omits companies from the payload when the array is empty', async () => {
    const form = makeForm({ companies: [] });

    const { result } = renderHookWithClient(() => useScraping(noopNotify, form));

    await act(async () => {
      await result.current.startScrape();
    });

    expect(mutateAsync).toHaveBeenCalledOnce();
    const payload = mutateAsync.mock.calls[0]?.[0] as Record<string, unknown>;
    expect(payload).not.toHaveProperty('companies');
  });

  it('includes companies in the payload when the array is non-empty', async () => {
    const form = makeForm({ companies: ['stripe', 'airbnb'] });

    const { result } = renderHookWithClient(() => useScraping(noopNotify, form));

    await act(async () => {
      await result.current.startScrape();
    });

    expect(mutateAsync).toHaveBeenCalledOnce();
    const payload = mutateAsync.mock.calls[0]?.[0] as Record<string, unknown>;
    expect(payload).toHaveProperty('companies', ['stripe', 'airbnb']);
  });
});

describe('useScraping — geo fields in the replace-vs-append signature', () => {
  it('replaces (not appends) when only the countryCode differs', async () => {
    const { result, rerender } = renderHookWithClient(
      ({ form }: { form: ScrapeFormState }) => useScraping(noopNotify, form),
      { initialProps: { form: makeForm({ countryCode: 'US' }) } }
    );

    // First run seeds the last-search signature.
    await act(async () => {
      await result.current.startScrape();
    });

    // Same keywords, different country → a different market must REPLACE the
    // stale results. This fails if countryCode is missing from the signature.
    rerender({ form: makeForm({ countryCode: 'DE' }) });
    await act(async () => {
      await result.current.startScrape();
    });

    expect(sentReplace(1)).toBe(true);
  });

  it('replaces (not appends) when only the search radius differs', async () => {
    const { result, rerender } = renderHookWithClient(
      ({ form }: { form: ScrapeFormState }) => useScraping(noopNotify, form),
      { initialProps: { form: makeForm({ radiusKm: 0 }) } }
    );

    await act(async () => {
      await result.current.startScrape();
    });

    // Same city, wider radius → a different search area must REPLACE. This
    // fails if radiusKm is missing from the signature.
    rerender({ form: makeForm({ radiusKm: 25 }) });
    await act(async () => {
      await result.current.startScrape();
    });

    expect(sentReplace(1)).toBe(true);
  });

  it('appends (does not replace) when the search is byte-for-byte identical', async () => {
    const { result, rerender } = renderHookWithClient(
      ({ form }: { form: ScrapeFormState }) => useScraping(noopNotify, form),
      { initialProps: { form: makeForm({ countryCode: 'US' }) } }
    );

    await act(async () => {
      await result.current.startScrape();
    });

    // Identical form (including geo) → "show more" semantics: keep + append.
    rerender({ form: makeForm({ countryCode: 'US' }) });
    await act(async () => {
      await result.current.startScrape();
    });

    // The first run REPLACES (nothing on screen belongs to it yet), the second
    // APPENDS — `replace` is omitted entirely from the payload.
    expect(sentReplace(0)).toBe(true);
    expect(sentReplace(1)).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// Defect 1 — DATA LOSS: "Show more" after a route change must not send
// replace:true (which clears the persisted postings cache on the first item).
// ---------------------------------------------------------------------------

describe('useScraping — the scrape survives a route change', () => {
  it('"Show more" after an unmount/remount APPENDS (no replace flag on the wire)', async () => {
    const form = makeForm({ query: 'engineer', amount: 25 });

    // 1. The user runs a search on the jobs page.
    const first = renderHookWithClient(() => useScraping(noopNotify, form));
    await act(async () => {
      await first.result.current.startScrape();
    });
    expect(sentReplace(0)).toBe(true);

    // 2. The user navigates to Settings — the page (and this hook) unmounts.
    first.unmount();

    // 3. …and comes back. Fresh component state, same session store.
    const second = renderHookWithClient(() => useScraping(noopNotify, form));

    // 4. "Show more": same criteria, a bigger amount. Contract: APPEND.
    await act(async () => {
      await second.result.current.startScrape(50);
    });

    expect(mutateAsync).toHaveBeenCalledTimes(2);
    expect(sentReplace(1)).toBeUndefined();
    const payload = mutateAsync.mock.calls[1]?.[0] as Record<string, unknown>;
    expect(payload).toMatchObject({ query: 'engineer', amount: 50 });
  });

  it('a genuinely different search after a remount still REPLACES', async () => {
    const first = renderHookWithClient(() => useScraping(noopNotify, makeForm({ query: 'rust' })));
    await act(async () => {
      await first.result.current.startScrape();
    });
    first.unmount();

    const second = renderHookWithClient(() =>
      useScraping(noopNotify, makeForm({ query: 'python' }))
    );
    await act(async () => {
      await second.result.current.startScrape();
    });

    expect(sentReplace(1)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Defect 2 — the in-flight scrape must stay cancellable across a route change.
// ---------------------------------------------------------------------------

describe('useScraping — the in-flight job survives a route change', () => {
  it('remounts into `scraping` and cancels the job the PREVIOUS mount started', async () => {
    const form = makeForm();

    const first = renderHookWithClient(() => useScraping(noopNotify, form));
    await act(async () => {
      await first.result.current.startScrape();
    });
    expect(first.result.current.scraping).toBe(true);
    first.unmount();

    const second = renderHookWithClient(() => useScraping(noopNotify, form));

    // Both the progress strip and the Cancel button are gated on `scraping`.
    expect(second.result.current.scraping).toBe(true);
    expect(second.result.current.scrapeJobId).toBe('j1');

    await act(async () => {
      await second.result.current.cancelScrape();
    });

    expect(cancelMutateAsync).toHaveBeenCalledWith('j1');
    expect(second.result.current.scraping).toBe(false);
    expect(second.result.current.scrapeJobId).toBeNull();
  });

  it('the next search after a remount cancels the orphaned scrape first', async () => {
    const form = makeForm();

    const first = renderHookWithClient(() => useScraping(noopNotify, form));
    await act(async () => {
      await first.result.current.startScrape();
    });
    first.unmount();

    // A second scrape from a fresh mount: without the stored job id this used
    // to start a rival run writing into the same postings cache.
    const second = renderHookWithClient(() =>
      useScraping(noopNotify, makeForm({ query: 'another' }))
    );
    await act(async () => {
      await second.result.current.startScrape();
    });

    expect(cancelMutateAsync).toHaveBeenCalledWith('j1');
    expect(cancelMutateAsync).toHaveBeenCalledTimes(1);
  });

  it('a remount onto a job that already FINISHED settles to a finished state', async () => {
    fetchJobMock.mockResolvedValue({ status: 'completed', result: { boards: [] } });
    vi.useFakeTimers();
    try {
      const form = makeForm();
      const first = renderHookWithClient(() => useScraping(noopNotify, form));
      await act(async () => {
        await first.result.current.startScrape();
      });
      first.unmount();

      // The job.completed EVENT is never delivered — the subscription lives on
      // the unmounted page. Only the watchdog can reconcile this.
      const second = renderHookWithClient(() => useScraping(noopNotify, form));
      expect(second.result.current.scraping).toBe(true);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(2600);
      });

      expect(second.result.current.scraping).toBe(false);
      expect(second.result.current.scrapeJobId).toBeNull();
      expect(second.result.current.scrapeOutcome).toEqual({ ok: true });
    } finally {
      vi.useRealTimers();
    }
  });
});

// ---------------------------------------------------------------------------
// Defect 3 — the per-board diagnostics ("aggregator: 429 rate limited") are the
// only explanation of an empty result; they must survive navigation AND a
// scrape that finishes while the user is on another route.
// ---------------------------------------------------------------------------

describe('useScraping — per-board diagnostics survive', () => {
  it('recovers the per-board summaries from the job tracker after finishing off-page', async () => {
    const boards = [{ board: 'aggregator', count: 0, error: '429 rate limited' }];
    fetchJobMock.mockResolvedValue({ status: 'completed', result: { count: 0, boards } });
    vi.useFakeTimers();
    try {
      const form = makeForm();
      const first = renderHookWithClient(() => useScraping(noopNotify, form));
      await act(async () => {
        await first.result.current.startScrape();
      });
      first.unmount();

      renderHookWithClient(() => useScraping(noopNotify, form));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2600);
      });

      expect(useSessionStore.getState().jobs.scrapeSummaries).toEqual(boards);
    } finally {
      vi.useRealTimers();
    }
  });
});
