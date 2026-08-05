/**
 * use-ai-provider — service tests
 *
 * 1. Smoke exercise — every exported hook renders without crashing.
 * 2. `{error}`-union rejection (review #16 HIGH): `setActiveProvider` /
 *    `setProviderSettings` RESOLVE (never reject) an `{ error }` union on a
 *    server-side rejection (e.g. an SSRF/provenance-rejected base_url). Each
 *    mutation hook must narrow that union and throw so React Query's
 *    `onError`/the caller's `catch` fire instead of a false `onSuccess`.
 * 3. `useConfigureActiveProvider` must STOP before `setActiveProvider` when the
 *    `setProviderSettings` half of the combined flow rejects.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import type { AppClient } from '@/lib/app-client';
import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import * as mod from './use-ai-provider';

describe('use-ai-provider services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

describe('fetchProviderModelsWithCache', () => {
  afterEach(() => {
    localStorage.clear();
  });

  function fakeClient(listProviderModels: AppClient['ai']['listProviderModels']): AppClient {
    return { ai: { listProviderModels } } as unknown as AppClient;
  }

  it('caches the list on a successful live fetch and returns it as fresh (not cached)', async () => {
    const models = [{ name: 'gpt-4o' }];
    const client = fakeClient(vi.fn().mockResolvedValue(models));

    const result = await mod.fetchProviderModelsWithCache(client, 'openai');

    expect(result).toEqual({ models, cached: false });
    expect(localStorage.getItem('ajh:model-list-cache:openai:')).toBe(JSON.stringify(models));
  });

  it('falls back to the last-good cache when the live fetch fails', async () => {
    // Seed the cache the way a prior successful fetch would have.
    await mod.fetchProviderModelsWithCache(
      fakeClient(vi.fn().mockResolvedValue([{ name: 'cached-model' }])),
      'anthropic'
    );

    const failing = fakeClient(
      vi.fn().mockRejectedValue(new Error('invalid or unauthorized API key'))
    );
    const result = await mod.fetchProviderModelsWithCache(failing, 'anthropic');

    expect(result).toEqual({ models: [{ name: 'cached-model' }], cached: true });
  });

  it('rejects — does NOT swallow the failure as a cache hit — when the cache holds only an empty list', async () => {
    // Seed the cache with a genuinely empty catalogue (PR 1's `Ok([])` case,
    // e.g. LM Studio with nothing loaded) — a real, once-successful result,
    // but not a usable fallback for THIS failure.
    await mod.fetchProviderModelsWithCache(fakeClient(vi.fn().mockResolvedValue([])), 'openai');
    expect(localStorage.getItem('ajh:model-list-cache:openai:')).toBe('[]');

    const failing = fakeClient(vi.fn().mockRejectedValue(new Error('revoked key')));

    await expect(mod.fetchProviderModelsWithCache(failing, 'openai')).rejects.toThrow(
      'revoked key'
    );
  });

  it('rejects with the original error when the live fetch fails and there is no cache', async () => {
    const client = fakeClient(vi.fn().mockRejectedValue(new Error('network error')));

    await expect(mod.fetchProviderModelsWithCache(client, 'gemini')).rejects.toThrow(
      'network error'
    );
  });
});

describe('useSetActiveProvider — {error}-union rejection', () => {
  it('rejects when setActiveProvider resolves { error }', async () => {
    const client = createMockClient({
      'ai.setActiveProvider': vi.fn().mockResolvedValue({ error: 'invalid provider' }),
    });
    const { result } = renderHookWithClient(() => mod.useSetActiveProvider(), { client });

    await expect(result.current.mutateAsync('bogus')).rejects.toThrow('invalid provider');
    await waitFor(() => expect(result.current.isError).toBe(true));
  });

  it('succeeds when setActiveProvider resolves the fresh config', async () => {
    const client = createMockClient({
      'ai.setActiveProvider': vi.fn().mockResolvedValue({ providers: {} }),
    });
    const { result } = renderHookWithClient(() => mod.useSetActiveProvider(), { client });

    await act(async () => {
      await result.current.mutateAsync('ollama');
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});

describe('useSetProviderSettings — {error}-union rejection', () => {
  it('rejects when setProviderSettings resolves { error }', async () => {
    const client = createMockClient({
      'ai.setProviderSettings': vi.fn().mockResolvedValue({ error: 'base_url rejected' }),
    });
    const { result } = renderHookWithClient(() => mod.useSetProviderSettings(), { client });

    await expect(
      result.current.mutateAsync({
        provider: 'openai-compatible',
        baseUrl: 'http://169.254.169.254',
      })
    ).rejects.toThrow('base_url rejected');
    await waitFor(() => expect(result.current.isError).toBe(true));
  });

  it('succeeds when setProviderSettings resolves the fresh config', async () => {
    const client = createMockClient({
      'ai.setProviderSettings': vi.fn().mockResolvedValue({ providers: {} }),
    });
    const { result } = renderHookWithClient(() => mod.useSetProviderSettings(), { client });

    await act(async () => {
      await result.current.mutateAsync({ provider: 'openai', model: 'gpt-4o' });
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});

describe('useConfigureActiveProvider — stops before setActiveProvider on a rejected settings write', () => {
  it('does NOT call setActiveProvider when setProviderSettings returns { error }', async () => {
    const setActiveProvider = vi.fn().mockResolvedValue({ providers: {} });
    const client = createMockClient({
      'ai.setProviderSettings': vi.fn().mockResolvedValue({ error: 'bad base_url' }),
      'ai.setActiveProvider': setActiveProvider,
    });
    const { result } = renderHookWithClient(() => mod.useConfigureActiveProvider(), { client });

    await expect(result.current.mutateAsync({ provider: 'openai-compatible' })).rejects.toThrow(
      'bad base_url'
    );

    expect(setActiveProvider).not.toHaveBeenCalled();
    await waitFor(() => expect(result.current.isError).toBe(true));
  });

  it('calls setActiveProvider when setProviderSettings succeeds', async () => {
    const setActiveProvider = vi.fn().mockResolvedValue({ providers: {} });
    const client = createMockClient({
      'ai.setProviderSettings': vi.fn().mockResolvedValue({ providers: {} }),
      'ai.setActiveProvider': setActiveProvider,
    });
    const { result } = renderHookWithClient(() => mod.useConfigureActiveProvider(), { client });

    await act(async () => {
      await result.current.mutateAsync({ provider: 'openai', model: 'gpt-4o' });
    });

    expect(setActiveProvider).toHaveBeenCalledOnce();
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });

  it('rejects when setActiveProvider itself returns { error } after a successful settings write', async () => {
    const client = createMockClient({
      'ai.setProviderSettings': vi.fn().mockResolvedValue({ providers: {} }),
      'ai.setActiveProvider': vi.fn().mockResolvedValue({ error: 'invalid provider' }),
    });
    const { result } = renderHookWithClient(() => mod.useConfigureActiveProvider(), { client });

    await expect(result.current.mutateAsync({ provider: 'bogus' })).rejects.toThrow(
      'invalid provider'
    );
    await waitFor(() => expect(result.current.isError).toBe(true));
  });
});
