/** Render helpers for the `useReferralDraft` tests (kept apart from `draft.test-support.ts`, see its header). */
import { createElement, type ReactNode } from 'react';
import { act, renderHook } from '@testing-library/react';

import { BASE } from './draft.test-support';
import { useReferralDraft } from './useReferralDraft';

// No QueryClient needed — useReferralDraft holds no React Query state.
export const wrapper = ({ children }: { children: ReactNode }) =>
  createElement('div', {}, children);
export const render = (p: typeof BASE = BASE) => renderHook(() => useReferralDraft(p), { wrapper });

/** Runs an async hook action inside `act` and awaits it. */
export const settle = (fn: () => unknown) =>
  act(async () => {
    await fn();
  });
