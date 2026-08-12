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
import { usePreferencesStore } from '@/store/preferences-store';
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

  it('sends the window stored for the model it activates', async () => {
    usePreferencesStore.getState().setLocalModelLimits('qwen3:8b', { contextWindow: 12_288 });
    const setProviderSettings = vi.fn().mockResolvedValue({ providers: {} });
    const client = createMockClient({
      'ai.setProviderSettings': setProviderSettings,
      'ai.setActiveProvider': vi.fn().mockResolvedValue({ providers: {} }),
    });
    const { result } = renderHookWithClient(() => mod.useConfigureActiveProvider(), { client });

    await act(async () => {
      await result.current.mutateAsync({ provider: 'ollama', model: 'qwen3:8b' });
    });

    expect(setProviderSettings).toHaveBeenCalledWith(
      expect.objectContaining({ provider: 'ollama', model: 'qwen3:8b', contextWindow: 12_288 })
    );
  });
});

/**
 * The REPLACE round-trip (the wiring defect this hook exists for): the backend
 * writes all four fields on every save, so a UI save that changes ONE field
 * must still carry the other three. Asserted on the payload that reaches the
 * client, which is the thing the backend sees.
 */
describe('useSaveProviderSettings — REPLACE round-trip', () => {
  afterEach(() => {
    // `resetPreferences` spreads the defaults, which have no `aiProviderConfig`
    // key at all — so it cannot clear one. Drop it explicitly or the local
    // limits set by one case leak into the next.
    usePreferencesStore.setState({ aiProviderConfig: undefined });
  });

  /** The writer merges against the LOADED active config, so every case has to
   *  wait for that query rather than for the mere fact that it was called. */
  const renderSaver = (client: AppClient) =>
    renderHookWithClient(
      () => ({ config: mod.useActiveConfig(), saver: mod.useSaveProviderSettings() }),
      { client }
    );

  it('keeps the window (and model) when an unrelated field is saved', async () => {
    // 1. The user sets a window for the model — LocalModelLimits' slider write.
    usePreferencesStore.getState().setLocalModelLimits('local-model', { contextWindow: 16_384 });

    const setProviderSettings = vi.fn().mockResolvedValue({ providers: {} });
    const client = createMockClient({
      'ai.setProviderSettings': setProviderSettings,
      // The backend row has NO window yet — the state every existing install is
      // in, since the slider only ever wrote renderer preferences.
      'ai.activeConfig': vi.fn().mockResolvedValue({
        activeProvider: 'ollama',
        providers: { ollama: { model: 'local-model' } },
      }),
    });
    const { result } = renderSaver(client);
    // The stored row has to have loaded before a save can preserve it.
    await waitFor(() => expect(result.current.config.isSuccess).toBe(true));

    // 2. Save something else entirely for the same provider row.
    await act(async () => {
      result.current.saver.save({ provider: 'ollama', model: 'local-model' });
      await Promise.resolve();
    });

    // 3. The window survived.
    await waitFor(() =>
      expect(setProviderSettings).toHaveBeenCalledWith({
        provider: 'ollama',
        model: 'local-model',
        baseUrl: undefined,
        contextWindow: 16_384,
      })
    );
  });

  it('keeps the stored model when only the base URL is saved', async () => {
    const setProviderSettings = vi.fn().mockResolvedValue({ providers: {} });
    const client = createMockClient({
      'ai.setProviderSettings': setProviderSettings,
      'ai.activeConfig': vi.fn().mockResolvedValue({
        activeProvider: 'openai-compatible',
        providers: { 'openai-compatible': { model: 'mistral-7b', baseUrl: 'http://old' } },
      }),
    });
    const { result } = renderSaver(client);
    await waitFor(() => expect(result.current.config.isSuccess).toBe(true));

    await act(async () => {
      result.current.saver.save({ provider: 'openai-compatible', baseUrl: 'http://new:1234/v1' });
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(setProviderSettings).toHaveBeenCalledWith(
        expect.objectContaining({ model: 'mistral-7b', baseUrl: 'http://new:1234/v1' })
      )
    );
  });

  it('re-sends a window the backend already holds when the renderer has no local limit', async () => {
    const setProviderSettings = vi.fn().mockResolvedValue({ providers: {} });
    const client = createMockClient({
      'ai.setProviderSettings': setProviderSettings,
      'ai.activeConfig': vi.fn().mockResolvedValue({
        activeProvider: 'ollama',
        providers: { ollama: { model: 'local-model', contextWindow: 20_480 } },
      }),
    });
    const { result } = renderSaver(client);
    await waitFor(() => expect(result.current.config.isSuccess).toBe(true));

    await act(async () => {
      result.current.saver.save({ provider: 'ollama' });
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(setProviderSettings).toHaveBeenCalledWith(
        expect.objectContaining({ model: 'local-model', contextWindow: 20_480 })
      )
    );
  });
});
