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

    // Wait for the openai-compatible key-status query to genuinely settle
    // (a real steady state) rather than a fixed sleep before the negative
    // assertion, which fails in the direction that looks like success.
    await waitFor(() =>
      expect(
        queryClient.getQueryState(['ai', 'models', 'provider-key', 'openai-compatible'])?.status
      ).toBe('success')
    );

    expect(listProviderModels).not.toHaveBeenCalled();
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
