/**
 * useProviderKeys — model-fetch gating tests (CodeRabbit #936, Major 2).
 *
 * `expandedCanFetchModels` used to gate purely on `keyStatus`, so expanding a
 * keyless `openai-compatible` row (LM Studio / vLLM — #935 made the backend
 * list models for it without a bearer header) never fired the catalogue
 * fetch at all, even though `ModelSelector` was already unblocked for the
 * same case. Every other cloud provider still requires a stored key.
 *
 * Uses `renderHookWithClient` (real QueryClient + AppClient, mock transport)
 * rather than mocking `@/services`, so the actual `enabled` wiring — not a
 * stubbed return value — is what's under test.
 */
import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, renderHookWithClient } from '@/test-support';

// useProviderKeys calls useNotification (@ajh/ui) directly — stub it (mirrors
// CloudProviderPanel.verify.test.tsx) rather than mounting NotificationProvider.
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...(actual as object),
    useNotification: () => ({
      success: vi.fn(),
      error: vi.fn(),
      warning: vi.fn(),
      info: vi.fn(),
      open: vi.fn(),
      destroy: vi.fn(),
    }),
  };
});

import { useProviderKeys } from './useProviderKeys';

describe('useProviderKeys — expanded-row model fetch gating', () => {
  it('fetches models for a keyless openai-compatible row with no stored key', async () => {
    const listProviderModels = vi.fn().mockResolvedValue([]);
    const client = createMockClient({
      'ai.listProviderModels': listProviderModels,
      'ai.activeConfig': vi.fn().mockResolvedValue({ activeProvider: 'ollama', providers: {} }),
      'ai.hasProviderKey': vi.fn().mockResolvedValue({ has: false }),
      'ai.listModels': vi.fn().mockResolvedValue([]),
      'system.health': vi.fn().mockResolvedValue({ ai: { ready: false }, cliAgents: {} }),
    });
    const { result } = renderHookWithClient(() => useProviderKeys(), { client });

    act(() => {
      result.current.toggleExpand('openai-compatible');
    });

    await waitFor(() =>
      expect(listProviderModels).toHaveBeenCalledWith({
        provider: 'openai-compatible',
        baseUrl: undefined,
      })
    );
  });

  it('does NOT fetch models for a key-required cloud provider with no stored key', async () => {
    const listProviderModels = vi.fn().mockResolvedValue([]);
    const client = createMockClient({
      'ai.listProviderModels': listProviderModels,
      'ai.activeConfig': vi.fn().mockResolvedValue({ activeProvider: 'ollama', providers: {} }),
      'ai.hasProviderKey': vi.fn().mockResolvedValue({ has: false }),
      'ai.listModels': vi.fn().mockResolvedValue([]),
      'system.health': vi.fn().mockResolvedValue({ ai: { ready: false }, cliAgents: {} }),
    });
    const { result, queryClient } = renderHookWithClient(() => useProviderKeys(), { client });

    act(() => {
      result.current.toggleExpand('openai');
    });

    // A fixed sleep before a negative assertion is a flake waiting for a slow
    // runner — it fails in the direction that looks like success. Wait for
    // the key-status query itself to genuinely settle instead: once its
    // network round trip resolves, `expandedCanFetchModels` has reached its
    // final value for the "no key, key-required provider" case (nothing else
    // in the chain is still in flight), so it's a real steady state to
    // assert against, not a guessed one.
    await waitFor(() =>
      expect(queryClient.getQueryState(['ai', 'models', 'provider-key', 'openai'])?.status).toBe(
        'success'
      )
    );

    expect(listProviderModels).not.toHaveBeenCalled();
  });
});
