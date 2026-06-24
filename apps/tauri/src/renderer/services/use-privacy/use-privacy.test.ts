/**
 * use-privacy — service tests
 *
 * Two sections:
 *  1. Smoke exercise — every exported hook renders without crashing.
 *  2. useResetApp success path — verifies clearOnboardingMirror() is called
 *     after api.privacy.resetApp() succeeds, alongside resetPreferences().
 *
 * vi.hoisted() is used for mock spies that are referenced inside vi.mock()
 * factories to avoid TDZ errors after hoisting.
 */
import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import { useInteractions } from '../use-postings/use-postings';
import * as mod from './use-privacy';
import { useClearInteractions } from './use-privacy';

// ── hoisted spies ─────────────────────────────────────────────────────────────

const { mockClearOnboardingMirror, mockResetPreferences } = vi.hoisted(() => ({
  mockClearOnboardingMirror: vi.fn().mockResolvedValue(undefined),
  mockResetPreferences: vi.fn(),
}));

// ── onboarding-mirror mock ─────────────────────────────────────────────────────

vi.mock('@/lib/onboarding-mirror', () => ({
  clearOnboardingMirror: mockClearOnboardingMirror,
  markOnboardingComplete: vi.fn().mockResolvedValue(undefined),
}));

// ── preferences-store mock — isolate from Zustand localStorage side-effects ───

vi.mock('@/store/preferences-store', () => ({
  usePreferencesStore: (selector: (s: { resetPreferences: () => void }) => unknown) =>
    selector({ resetPreferences: mockResetPreferences }),
}));

// ── smoke ─────────────────────────────────────────────────────────────────────

describe('use-privacy services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

// ── useResetApp — clearOnboardingMirror called on success ─────────────────────

describe('useResetApp', () => {
  it('calls clearOnboardingMirror() on successful resetApp', async () => {
    mockClearOnboardingMirror.mockClear();
    mockResetPreferences.mockClear();

    const client = createMockClient({
      'privacy.resetApp': vi.fn().mockResolvedValue(undefined),
    });

    const { result } = renderHookWithClient(() => mod.useResetApp(), { client });

    await act(async () => {
      result.current.mutate();
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(mockClearOnboardingMirror).toHaveBeenCalledOnce();
    expect(mockResetPreferences).toHaveBeenCalledOnce();
  });

  it('does NOT call clearOnboardingMirror() or resetPreferences() when resetApp fails', async () => {
    mockClearOnboardingMirror.mockClear();
    mockResetPreferences.mockClear();

    const client = createMockClient({
      'privacy.resetApp': vi.fn().mockRejectedValue(new Error('network error')),
    });

    const { result } = renderHookWithClient(() => mod.useResetApp(), { client });

    await act(async () => {
      result.current.mutate();
    });

    await waitFor(() => {
      expect(result.current.isError).toBe(true);
    });

    expect(mockClearOnboardingMirror).not.toHaveBeenCalled();
    expect(mockResetPreferences).not.toHaveBeenCalled();
  });
});

// ── useClearInteractions — prefix invalidation refetches typed interactions ────

describe('useClearInteractions — interactions prefix invalidation', () => {
  /**
   * Real-chain test: uses the REAL useClearInteractions + REAL useInteractions
   * hooks with only the IPC client mocked. Verifies that after clearInteractions
   * settles, the interactions query for a typed key ('viewed') refetches — i.e.
   * the prefix ['postings','interactions'] hits ['postings','interactions','viewed'].
   *
   * This guards against the prior bug where invalidating the full key
   * ['postings','interactions',undefined] did NOT match typed queries and stale
   * "viewed/saved" badges lingered until reload.
   */
  it('refetches useInteractions("viewed") after clearInteractions mutates', async () => {
    const listInteractions = vi.fn().mockResolvedValue([]);
    const clearInteractions = vi.fn().mockResolvedValue(undefined);

    const client = createMockClient({
      'scrape.listInteractions': listInteractions,
      'privacy.clearInteractions': clearInteractions,
    });

    const { result } = renderHookWithClient(
      () => ({
        interactions: useInteractions('viewed'),
        clear: useClearInteractions(),
      }),
      { client }
    );

    // Wait for the initial interactions query to settle.
    await waitFor(() => expect(result.current.interactions.isSuccess).toBe(true));
    const callCountBefore = listInteractions.mock.calls.length;

    // Trigger the mutation.
    await act(async () => {
      await result.current.clear.mutateAsync();
    });

    // The interactions query for 'viewed' must have re-fired (prefix invalidation).
    await waitFor(() => {
      expect(listInteractions.mock.calls.length).toBeGreaterThan(callCountBefore);
    });

    // The refetch must have been for the 'viewed' type.
    const refetchCall = listInteractions.mock.calls[callCountBefore];
    expect(refetchCall?.[0]).toMatchObject({ interactionType: 'viewed' });
  });

  it('does not refetch interactions when clearInteractions rejects', async () => {
    const listInteractions = vi.fn().mockResolvedValue([]);
    const clearInteractions = vi.fn().mockRejectedValue(new Error('backend error'));

    const client = createMockClient({
      'scrape.listInteractions': listInteractions,
      'privacy.clearInteractions': clearInteractions,
    });

    const { result } = renderHookWithClient(
      () => ({
        interactions: useInteractions('viewed'),
        clear: useClearInteractions(),
      }),
      { client }
    );

    await waitFor(() => expect(result.current.interactions.isSuccess).toBe(true));
    const callCountBefore = listInteractions.mock.calls.length;

    // 1. The mutation must actually reject — proves the error path was exercised.
    await expect(result.current.clear.mutateAsync()).rejects.toThrow('backend error');

    // 2. clearInteractions was called — the mutation ran, it didn't silently no-op.
    expect(clearInteractions).toHaveBeenCalledOnce();

    // 3. onSuccess was skipped — listInteractions did not refetch.
    await waitFor(() => expect(result.current.clear.isError).toBe(true));
    expect(listInteractions.mock.calls.length).toBe(callCountBefore);
  });
});
