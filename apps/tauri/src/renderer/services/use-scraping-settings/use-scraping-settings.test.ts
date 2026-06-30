/**
 * use-scraping-settings — unit tests.
 *
 * Covers:
 *  - useScrapingSettings: returns defaults when store is empty.
 *  - useScrapingSettings: maps stored boolean + string values.
 *  - useScrapingSettings: returns defaults when Store.load rejects (error resilience).
 *  - useUpdateScrapingSettings: sets apifyLinkedinEnabled and calls save.
 *  - useUpdateScrapingSettings: deletes actorId when patch value is empty string.
 *  - useUpdateScrapingSettings: sets actorId when patch value is non-empty.
 *  - useUpdateScrapingSettings: does not touch actorId key when not in patch.
 *
 * @tauri-apps/plugin-store is fully mocked — no Tauri runtime needed.
 */
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';

import { SCRAPING_SETTINGS_KEYS } from '@ajh/shared';

// ── Plugin-store mock ─────────────────────────────────────────────────────────

const { mockGet, mockSet, mockDelete, mockSave, mockStoreLoad } = vi.hoisted(() => {
  const mockGet = vi.fn().mockResolvedValue(undefined);
  const mockSet = vi.fn().mockResolvedValue(undefined);
  const mockDelete = vi.fn().mockResolvedValue(undefined);
  const mockSave = vi.fn().mockResolvedValue(undefined);
  const mockStoreLoad = vi.fn().mockResolvedValue({
    get: mockGet,
    set: mockSet,
    delete: mockDelete,
    save: mockSave,
  });
  return { mockGet, mockSet, mockDelete, mockSave, mockStoreLoad };
});

vi.mock('@tauri-apps/plugin-store', () => ({
  Store: { load: mockStoreLoad },
}));

// ── Test helper ───────────────────────────────────────────────────────────────

function makeWrapper() {
  const qc = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: qc }, children);
  return { qc, wrapper };
}

afterEach(() => {
  vi.clearAllMocks();
  // Re-apply default implementations cleared by clearAllMocks.
  mockGet.mockResolvedValue(undefined);
  mockSet.mockResolvedValue(undefined);
  mockDelete.mockResolvedValue(undefined);
  mockSave.mockResolvedValue(undefined);
  mockStoreLoad.mockResolvedValue({
    get: mockGet,
    set: mockSet,
    delete: mockDelete,
    save: mockSave,
  });
});

// ── Lazy imports (after vi.mock hoisting) ─────────────────────────────────────

import { useScrapingSettings, useUpdateScrapingSettings } from './use-scraping-settings';

// ── useScrapingSettings ───────────────────────────────────────────────────────

describe('useScrapingSettings', () => {
  it('returns defaults when store is empty', async () => {
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useScrapingSettings(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({
      apifyLinkedinEnabled: false,
      apifyLinkedinActorId: undefined,
    });
  });

  it('maps stored boolean + string values', async () => {
    mockGet.mockImplementation(async (key: string) => {
      if (key === SCRAPING_SETTINGS_KEYS.apifyLinkedinEnabled) return true;
      if (key === SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId) return 'custom~actor';
      return undefined;
    });
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useScrapingSettings(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({
      apifyLinkedinEnabled: true,
      apifyLinkedinActorId: 'custom~actor',
    });
  });

  it('returns defaults when Store.load rejects (error resilience)', async () => {
    mockStoreLoad.mockRejectedValueOnce(new Error('FS permission denied'));
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useScrapingSettings(), { wrapper });
    // Error inside queryFn is caught; query resolves to defaults.
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({
      apifyLinkedinEnabled: false,
      apifyLinkedinActorId: undefined,
    });
  });

  it('trims whitespace-only actorId to undefined', async () => {
    mockGet.mockImplementation(async (key: string) => {
      if (key === SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId) return '   ';
      return undefined;
    });
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useScrapingSettings(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.apifyLinkedinActorId).toBeUndefined();
  });
});

// ── useUpdateScrapingSettings ─────────────────────────────────────────────────

describe('useUpdateScrapingSettings', () => {
  it('sets apifyLinkedinEnabled and calls save', async () => {
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useUpdateScrapingSettings(), { wrapper });
    await act(async () => {
      await result.current.mutateAsync({ apifyLinkedinEnabled: true });
    });
    expect(mockSet).toHaveBeenCalledWith(SCRAPING_SETTINGS_KEYS.apifyLinkedinEnabled, true);
    expect(mockSave).toHaveBeenCalledOnce();
  });

  it('deletes actorId when patch value is empty string', async () => {
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useUpdateScrapingSettings(), { wrapper });
    await act(async () => {
      await result.current.mutateAsync({ apifyLinkedinActorId: '' });
    });
    expect(mockDelete).toHaveBeenCalledWith(SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId);
    expect(mockSet).not.toHaveBeenCalledWith(
      SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId,
      expect.anything()
    );
    expect(mockSave).toHaveBeenCalledOnce();
  });

  it('sets actorId when patch value is non-empty', async () => {
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useUpdateScrapingSettings(), { wrapper });
    await act(async () => {
      await result.current.mutateAsync({ apifyLinkedinActorId: 'custom~actor' });
    });
    expect(mockSet).toHaveBeenCalledWith(
      SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId,
      'custom~actor'
    );
    expect(mockDelete).not.toHaveBeenCalled();
    expect(mockSave).toHaveBeenCalledOnce();
  });

  it('does not touch actorId key when it is absent from the patch', async () => {
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useUpdateScrapingSettings(), { wrapper });
    await act(async () => {
      await result.current.mutateAsync({ apifyLinkedinEnabled: false });
    });
    expect(mockSet).not.toHaveBeenCalledWith(
      SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId,
      expect.anything()
    );
    expect(mockDelete).not.toHaveBeenCalled();
    expect(mockSave).toHaveBeenCalledOnce();
  });
});
