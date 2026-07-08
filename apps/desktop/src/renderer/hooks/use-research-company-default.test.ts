import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, renderHookWithClient } from '@/test-support';

import { useResearchCompanyDefault } from './use-research-company-default';

function clientWithWebSearch(supportsWebSearch: boolean) {
  return createMockClient({
    'ai.modelCapabilities': vi.fn().mockResolvedValue({ supportsWebSearch }),
  });
}

/**
 * A client whose capability answer depends on the requested model — lets a test
 * drive a mid-session model switch (via the preferences store) and observe the
 * default follow it. Only `searcher` can web-search.
 */
function clientByModel() {
  return createMockClient({
    'ai.modelCapabilities': vi.fn(({ model }: { model: string }) =>
      Promise.resolve({ supportsWebSearch: model === 'searcher' })
    ),
  });
}

/** Point the active provider/model at `model` (drives the capability query key). */
function useModel(model: string) {
  act(() =>
    usePreferencesStore.getState().setAiProviderConfig({
      activeProvider: 'ollama',
      providers: { ollama: { model } },
    })
  );
}

/** Let the capability query resolve and its effect apply (macrotask settle). */
async function settle() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

// The preferences store is a persisted singleton — reset the model config so each
// test starts from a known active provider/model.
beforeEach(() => {
  usePreferencesStore.setState({ aiProviderConfig: undefined });
});

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

  it('re-seeds when the model changes mid-session while still untouched', async () => {
    useModel('plain');
    const { result } = renderHookWithClient(() => useResearchCompanyDefault(), {
      client: clientByModel(),
    });
    await settle();
    expect(result.current[0]).toBe(false);

    // Switch to a web-search-capable model — the untouched default follows it.
    useModel('searcher');
    await waitFor(() => expect(result.current[0]).toBe(true));

    // Switch back to a plain model — the default follows again.
    useModel('plain');
    await waitFor(() => expect(result.current[0]).toBe(false));
  });

  it('keeps the user override across a mid-session model change', async () => {
    useModel('searcher');
    const { result } = renderHookWithClient(() => useResearchCompanyDefault(), {
      client: clientByModel(),
    });
    await waitFor(() => expect(result.current[0]).toBe(true));

    // User turns it OFF — an explicit choice.
    act(() => result.current[1](false));
    expect(result.current[0]).toBe(false);

    // A later model switch must NOT resurrect the capability default.
    useModel('plain');
    await settle();
    expect(result.current[0]).toBe(false);
  });
});
