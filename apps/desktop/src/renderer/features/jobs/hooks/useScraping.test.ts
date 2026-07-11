/**
 * useScraping — companies payload guard.
 *
 * Exercises the actual hook (not a local clone) to verify that the
 * `companies` field is conditionally included in the scrapeBoards payload
 * so the IPC contract is honoured and the Rust engine's is_empty() skip
 * check behaves correctly.
 */
import { describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import type { useNotification } from '@ajh/ui';

import { renderHookWithClient } from '@/test-support';

// ---------------------------------------------------------------------------
// Stubs — must be declared before imports that trigger the module under test.
// ---------------------------------------------------------------------------

const mutateAsync = vi.fn().mockResolvedValue({ jobId: 'j1' });

vi.mock('@/services', async (importActual) => {
  const actual = await importActual<Record<string, unknown>>();
  return {
    ...actual,
    useScrapeBoards: () => ({ mutateAsync }),
    useCancelJob: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    fetchJob: vi.fn().mockResolvedValue({ status: 'completed' }),
    useInvalidatePostings: () => vi.fn().mockResolvedValue(undefined),
  };
});

// Import under test AFTER mocks.
import type { ScrapeFormState } from '../components/ScrapeForm/constants';
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('useScraping — companies field in scrapeBoards payload', () => {
  it('omits companies from the payload when the array is empty', async () => {
    mutateAsync.mockClear();
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
    mutateAsync.mockClear();
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
    mutateAsync.mockClear();
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

    expect(result.current.replacePendingRef.current).toBe(true);
  });

  it('replaces (not appends) when only the search radius differs', async () => {
    mutateAsync.mockClear();
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

    expect(result.current.replacePendingRef.current).toBe(true);
  });

  it('appends (does not replace) when the search is byte-for-byte identical', async () => {
    mutateAsync.mockClear();
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

    expect(result.current.replacePendingRef.current).toBe(false);
  });
});
