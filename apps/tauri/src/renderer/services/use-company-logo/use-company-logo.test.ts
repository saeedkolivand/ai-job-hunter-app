/**
 * useCompanyLogo — defensive logo resolution via Clearbit Autocomplete.
 *
 * Strategy:
 *  - global.fetch is mocked per test (vitest restores via afterEach).
 *  - QueryClient with retry:0 + staleTime:0 so failures surface immediately.
 *  - Covers: disabled (no fetch), happy path, empty results, fetch error, non-OK status.
 *
 * noUncheckedIndexedAccess: no bare array[0] accesses.
 */

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';

import { useCompanyLogo } from './use-company-logo';

function makeWrapper() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0, staleTime: 0 } },
  });
  return ({ children }: { children: React.ReactNode }) =>
    React.createElement(QueryClientProvider, { client: qc }, children);
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('useCompanyLogo — disabled', () => {
  it('makes no fetch calls when enabled=false', () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    const { result } = renderHook(() => useCompanyLogo('Acme', false), {
      wrapper: makeWrapper(),
    });
    expect(result.current).toBeNull();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('makes no fetch calls when company is empty string', () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    const { result } = renderHook(() => useCompanyLogo('', true), {
      wrapper: makeWrapper(),
    });
    expect(result.current).toBeNull();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('makes no fetch calls when company is whitespace only', () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    const { result } = renderHook(() => useCompanyLogo('   ', true), {
      wrapper: makeWrapper(),
    });
    expect(result.current).toBeNull();
    expect(fetchSpy).not.toHaveBeenCalled();
  });
});

describe('useCompanyLogo — happy path', () => {
  it('returns the first result logo URL on success', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify([
          { name: 'Acme Corp', domain: 'acme.com', logo: 'https://logo.clearbit.com/acme.com' },
        ]),
        { status: 200 }
      )
    );

    const { result } = renderHook(() => useCompanyLogo('Acme', true), {
      wrapper: makeWrapper(),
    });

    await waitFor(() => expect(result.current).not.toBeNull());
    expect(result.current).toBe('https://logo.clearbit.com/acme.com');
  });
});

describe('useCompanyLogo — failures return null (never throw)', () => {
  it('returns null when Clearbit returns an empty array', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify([]), { status: 200 })
    );

    const { result } = renderHook(() => useCompanyLogo('Unknown Co', true), {
      wrapper: makeWrapper(),
    });

    await waitFor(() => result.current === null);
    expect(result.current).toBeNull();
  });

  it('returns null on a non-OK HTTP response', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(null, { status: 429 }));

    const { result } = renderHook(() => useCompanyLogo('Acme', true), {
      wrapper: makeWrapper(),
    });

    await waitFor(() => result.current === null);
    expect(result.current).toBeNull();
  });

  it('returns null on a network error (fetch throws)', async () => {
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('network error'));

    const { result } = renderHook(() => useCompanyLogo('Acme', true), {
      wrapper: makeWrapper(),
    });

    await waitFor(() => result.current === null);
    expect(result.current).toBeNull();
  });

  it('returns null when the logo field is missing from the result', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify([{ name: 'Acme Corp', domain: 'acme.com' /* no logo */ }]), {
        status: 200,
      })
    );

    const { result } = renderHook(() => useCompanyLogo('Acme', true), {
      wrapper: makeWrapper(),
    });

    await waitFor(() => result.current === null);
    expect(result.current).toBeNull();
  });
});
