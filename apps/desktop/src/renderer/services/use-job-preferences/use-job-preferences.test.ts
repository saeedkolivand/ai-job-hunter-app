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
