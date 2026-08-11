import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import * as mod from './use-job-preferences';

describe('use-job-preferences services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

describe('salary expectation boot sync (review fix, PR #695)', () => {
  beforeEach(() => {
    usePreferencesStore.getState().resetPreferences();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('swallows a rejected setSalaryExpectation — no throw, no unhandled rejection', async () => {
    usePreferencesStore.getState().setApplicant({ salaryExpectation: '€75,000' });
    const setSalaryExpectation = vi.fn().mockRejectedValue(new Error('backend offline'));
    const client = createMockClient({
      'jobPreferences.setSalaryExpectation': setSalaryExpectation,
    });

    expect(() =>
      renderHookWithClient(() => mod.useSyncSalaryExpectation(), { client })
    ).not.toThrow();
    await waitFor(() => expect(setSalaryExpectation).toHaveBeenCalledExactlyOnceWith('€75,000'));
    // Let the rejected promise's `.catch` run — an unswallowed rejection here
    // would surface as an unhandled-rejection failure from the test runner,
    // not a thrown error this `await` could otherwise observe directly.
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
});

/**
 * The backend-readable `semanticScoring` mirror (ADR-020 addendum) — the ONLY
 * channel the headless Autopilot scheduler has for a setting that lives in the
 * webview's localStorage. If the boot sync stops firing, semantic scoring
 * silently never reaches a scheduled run.
 */
describe('semantic scoring boot sync', () => {
  beforeEach(() => {
    usePreferencesStore.getState().resetPreferences();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('pushes the persisted preference to the backend exactly once on mount', async () => {
    usePreferencesStore.getState().setSemanticScoring(true);
    const setSemanticScoring = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'jobPreferences.setSemanticScoring': setSemanticScoring });

    const { rerender } = renderHookWithClient(() => mod.useSyncSemanticScoring(), { client });
    await waitFor(() => expect(setSemanticScoring).toHaveBeenCalledExactlyOnceWith(true));
    rerender();
    expect(setSemanticScoring).toHaveBeenCalledTimes(1);
  });

  it('pushes FALSE too — "off" is the value that keeps a scheduled run embedding-free', async () => {
    // A skip-when-falsy guard (copied from the salary sync, where absence is
    // genuinely nothing to say) would leave a stale `true` in the backend
    // mirror after the user turns the setting off, and the scheduler would keep
    // embedding. This is the regression that guard would cause.
    usePreferencesStore.getState().setSemanticScoring(false);
    const setSemanticScoring = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'jobPreferences.setSemanticScoring': setSemanticScoring });

    renderHookWithClient(() => mod.useSyncSemanticScoring(), { client });
    await waitFor(() => expect(setSemanticScoring).toHaveBeenCalledExactlyOnceWith(false));
  });

  it('swallows a rejected push — no throw, no unhandled rejection', async () => {
    usePreferencesStore.getState().setSemanticScoring(true);
    const setSemanticScoring = vi.fn().mockRejectedValue(new Error('backend offline'));
    const client = createMockClient({ 'jobPreferences.setSemanticScoring': setSemanticScoring });

    expect(() =>
      renderHookWithClient(() => mod.useSyncSemanticScoring(), { client })
    ).not.toThrow();
    await waitFor(() => expect(setSemanticScoring).toHaveBeenCalledTimes(1));
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
});
