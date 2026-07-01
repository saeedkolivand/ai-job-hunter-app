import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { createMockClient } from '@/lib/mock-client';

import { AppClientProvider, useAppClient } from './AppClientProvider';

describe('AppClientProvider', () => {
  it('exposes the injected client via useAppClient', () => {
    const client = createMockClient();
    const { result } = renderHook(() => useAppClient(), {
      wrapper: ({ children }) => <AppClientProvider client={client}>{children}</AppClientProvider>,
    });
    expect(result.current).toBe(client);
  });

  it('throws when useAppClient is used outside the provider', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => renderHook(() => useAppClient())).toThrow(/within <AppClientProvider>/);
    vi.restoreAllMocks();
  });

  it('throws when rendered without a client prop', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() =>
      renderHook(() => useAppClient(), {
        wrapper: ({ children }) => <AppClientProvider>{children}</AppClientProvider>,
      })
    ).toThrow(/requires a client prop/);
    vi.restoreAllMocks();
  });
});
