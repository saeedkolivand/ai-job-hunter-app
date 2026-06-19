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

import * as mod from './use-privacy';

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
