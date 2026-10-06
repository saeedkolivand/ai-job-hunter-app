/** Render helpers for the `useApplicationAnswers` tests (kept apart from `mocks.ts`, see its header). */
import { createElement, type ReactNode } from 'react';
import { vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import { generateApplicationAnswer, lookupSalaryRange, researchAnswer } from '@/lib/generate';

import { useApplicationAnswers } from '../useApplicationAnswers';
import { save } from './mocks';

const base = {
  resume: 'My resume',
  jobDesc: 'Backend role at Acme',
  model: 'llama3',
  researchCompany: false,
  meta: null,
  canUse: true,
  hasDesc: true,
  jobUrl: 'https://acme.com/job/1',
  board: 'linkedin',
};

const wrapper = ({ children }: { children: ReactNode }) =>
  createElement(QueryClientProvider, { client: new QueryClient() }, children);

export const render = (overrides: Partial<Parameters<typeof useApplicationAnswers>[0]> = {}) =>
  renderHook(() => useApplicationAnswers({ ...base, ...overrides }), { wrapper });

/** Runs `generate()` inside `act` and awaits it. */
export const generate = (result: ReturnType<typeof render>['result']) =>
  act(async () => {
    await result.current.generate();
  });

/** Call in `beforeEach`. */
export function resetMocks() {
  save.mockClear();
  vi.mocked(generateApplicationAnswer).mockClear();
  vi.mocked(lookupSalaryRange).mockClear();
  vi.mocked(lookupSalaryRange).mockResolvedValue(undefined);
  vi.mocked(researchAnswer).mockClear();
  vi.mocked(researchAnswer).mockResolvedValue('');
}

/** Renders, selects the given question ids, runs `generate()`, returns the hook result. */
export async function generateSelected(id: string, overrides: Parameters<typeof render>[0] = {}) {
  const { result } = render(overrides);
  act(() => result.current.toggle(id));
  await generate(result);
  return result;
}
