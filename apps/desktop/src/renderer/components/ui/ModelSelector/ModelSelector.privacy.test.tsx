/**
 * ModelSelector — keyless-provider privacy regression tests
 * (fix/no-unconfigured-openai-probe).
 *
 * ModelSelector builds a `useQueries` model fetch for EVERY cloud provider on
 * every mount (`cloudProviders.map`), gated per-provider by `canFetchModels`.
 * The #936 carve-out — `connected || provider === 'openai-compatible'` — had
 * no requirement that `openai-compatible` be pointed anywhere, so a user who
 * never touched that provider (e.g. a fully-local Ollama user) still fired
 * `listProviderModels` for it on every page mounting this component; the Rust
 * side falls back to `https://api.openai.com/v1` when no base URL is
 * configured, sending an outbound request with no `Authorization` header.
 *
 * Unlike `ModelSelector.test.tsx` (which stubs `@tanstack/react-query`'s
 * `useQueries` entirely, bypassing `enabled`), this file mounts a real
 * QueryClient + a mocked AppClient transport — mirroring
 * `useProviderKeys.test.ts` — so the actual `enabled` wiring is what's under
 * test, not a stubbed query result.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

import { createMockClient, makeQueryClient, withProviders } from '@/test-support';

import { ModelSelector } from './index';

describe('ModelSelector — openai-compatible model-fetch gating', () => {
  it('never calls listProviderModels for openai-compatible with no stored key and no base URL', async () => {
    const listProviderModels = vi.fn().mockResolvedValue([]);
    const client = createMockClient({
      'ai.listProviderModels': listProviderModels,
      'ai.activeConfig': vi.fn().mockResolvedValue({ activeProvider: 'ollama', providers: {} }),
      'ai.hasProviderKey': vi.fn().mockResolvedValue({ has: false }),
      'ai.listModels': vi.fn().mockResolvedValue([]),
      'system.health': vi.fn().mockResolvedValue({ ai: { ready: true }, cliAgents: {} }),
    });
    const queryClient = makeQueryClient();

    render(<ModelSelector />, { wrapper: withProviders(client, queryClient) });

    // Wait for BOTH inputs `canFetchModels` reads to genuinely settle — the
    // key-status query AND the active-config query (the source of the base
    // URL half of the check, via `baseUrlFor`). Gating on the key query
    // alone would pass vacuously for a future variant that stubs a `baseUrl`
    // while still expecting no fetch: `activeConfig` could still be
    // in-flight at that point, so `baseUrlFor` would read `undefined` and the
    // assertion would pass for the wrong reason.
    await waitFor(() => {
      expect(
        queryClient.getQueryState(['ai', 'models', 'provider-key', 'openai-compatible'])?.status
      ).toBe('success');
      expect(queryClient.getQueryState(['ai', 'activeConfig'])?.status).toBe('success');
    });

    expect(listProviderModels).not.toHaveBeenCalled();
  });

  it('calls listProviderModels for openai-compatible when a key is stored, even with no base URL (PR #937 finding 2)', async () => {
    // The other half of "configured" — authenticated by a stored KEY instead
    // of a base URL. Must fetch same as any other cloud provider with a key;
    // this is the network-level twin of the message-level regression test in
    // ModelSelector.test.tsx (finding 2: the key query being irrelevant to
    // openai-compatible was hardcoded, not conditional on actually having a
    // base URL to fall back on).
    const listProviderModels = vi.fn().mockResolvedValue([]);
    const client = createMockClient({
      'ai.listProviderModels': listProviderModels,
      'ai.activeConfig': vi.fn().mockResolvedValue({ activeProvider: 'ollama', providers: {} }),
      'ai.hasProviderKey': vi.fn().mockResolvedValue({ has: true }),
      'ai.listModels': vi.fn().mockResolvedValue([]),
      'system.health': vi.fn().mockResolvedValue({ ai: { ready: true }, cliAgents: {} }),
    });
    const queryClient = makeQueryClient();

    render(<ModelSelector />, { wrapper: withProviders(client, queryClient) });

    await waitFor(() =>
      expect(listProviderModels).toHaveBeenCalledWith({
        provider: 'openai-compatible',
        baseUrl: undefined,
      })
    );
  });

  it('calls listProviderModels for openai-compatible once a base URL is configured, with no key', async () => {
    const listProviderModels = vi.fn().mockResolvedValue([]);
    const client = createMockClient({
      'ai.listProviderModels': listProviderModels,
      'ai.activeConfig': vi.fn().mockResolvedValue({
        activeProvider: 'ollama',
        providers: { 'openai-compatible': { baseUrl: 'http://localhost:1234/v1' } },
      }),
      'ai.hasProviderKey': vi.fn().mockResolvedValue({ has: false }),
      'ai.listModels': vi.fn().mockResolvedValue([]),
      'system.health': vi.fn().mockResolvedValue({ ai: { ready: true }, cliAgents: {} }),
    });
    const queryClient = makeQueryClient();

    render(<ModelSelector />, { wrapper: withProviders(client, queryClient) });

    await waitFor(() =>
      expect(listProviderModels).toHaveBeenCalledWith({
        provider: 'openai-compatible',
        baseUrl: 'http://localhost:1234/v1',
      })
    );
  });
});
