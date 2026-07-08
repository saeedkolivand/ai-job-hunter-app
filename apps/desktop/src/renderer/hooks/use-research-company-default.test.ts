import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, renderHookWithClient } from '@/test-support';

import { useResearchCompanyDefault } from './use-research-company-default';

function clientWithWebSearch(supportsWebSearch: boolean) {
  return createMockClient({
    'ai.modelCapabilities': vi.fn().mockResolvedValue({ supportsWebSearch }),
  });
}

/** Let the capability query resolve and its effect apply (macrotask settle). */
async function settle() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

describe('useResearchCompanyDefault', () => {
  it('defaults ON once a web-search-capable model resolves', async () => {
    const { result } = renderHookWithClient(() => useResearchCompanyDefault(), {
      client: clientWithWebSearch(true),
    });
    // Safe fallback while the capability is still resolving.
    expect(result.current[0]).toBe(false);
    await waitFor(() => expect(result.current[0]).toBe(true));
  });

  it('stays OFF for a model that cannot web-search', async () => {
    const { result } = renderHookWithClient(() => useResearchCompanyDefault(), {
      client: clientWithWebSearch(false),
    });
    await settle();
    expect(result.current[0]).toBe(false);
  });

  it('never clobbers an explicit user override on a late resolve', async () => {
    const { result } = renderHookWithClient(() => useResearchCompanyDefault(), {
      client: clientWithWebSearch(true),
    });
    // User turns it OFF before the capability resolves.
    act(() => result.current[1](false));
    // Capability then resolves to ON — but the user's choice must win.
    await settle();
    expect(result.current[0]).toBe(false);
  });
});
