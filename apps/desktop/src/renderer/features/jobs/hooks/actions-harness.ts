/**
 * Harness for the usePostingActions suites: stubs every service hook (the
 * `vi.mock` calls below apply to each suite that imports this module) and
 * exposes the spies plus the `setup` / `run` helpers.
 */
import { expect, type Mock, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

export const notifySuccess: Mock = vi.fn();
export const notifyError: Mock = vi.fn();

vi.mock('@ajh/ui', () => ({
  useNotification: () => ({ success: notifySuccess, error: notifyError }),
}));

export const mockNavigate: Mock = vi.fn().mockResolvedValue(undefined);

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

export const mockSetApplicationApply: Mock = vi.fn();

vi.mock('@/store/session-store', () => ({
  useSessionStore: (sel: (s: { setApplicationApply: typeof mockSetApplicationApply }) => unknown) =>
    sel({ setApplicationApply: mockSetApplicationApply }),
}));

export const mockOpenExternalAsync: Mock = vi.fn().mockResolvedValue(undefined);
export const mockPersistJobAsync: Mock = vi.fn().mockResolvedValue(undefined);
export const mockSaveFromPostingAsync: Mock = vi.fn().mockResolvedValue({ id: 'app-1' });

export const saveState = { isPending: false };

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutateAsync: mockOpenExternalAsync }),
  usePersistJob: () => ({ mutateAsync: mockPersistJobAsync }),
}));

vi.mock('@/services/use-applications', () => ({
  useSaveFromPosting: () => ({
    mutateAsync: mockSaveFromPostingAsync,
    get isPending() {
      return saveState.isPending;
    },
  }),
}));

// vi.fn() so individual tests can override via mockReturnValueOnce.
const noScore = { score: undefined, pending: false, hasResume: false };
export const mockUseRowMatchScore: Mock = vi.fn().mockReturnValue(noScore);

vi.mock('@/features/jobs/providers', () => ({
  useRowMatchScore: (...args: unknown[]) => mockUseRowMatchScore(...args),
}));

vi.mock('@/lib/match-level', () => ({
  scoreToLevel: (n: number) => (n >= 0.7 ? 'high' : 'medium'),
}));

import type { Posting } from '../types';
import { usePostingActions } from './usePostingActions';

function makePosting(overrides: Partial<Posting> = {}): Posting {
  return {
    id: 'post-1',
    source: 'linkedin',
    externalId: 'ext-1',
    url: 'https://example.com/job/1',
    title: 'Software Engineer',
    company: 'Acme',
    location: 'Berlin',
    description: 'Great role requiring Rust skills.',
    capturedAt: 1_700_000_000_000,
    ...overrides,
  };
}

/** A posting that already carries one interaction of each given type. */
type InteractionType = NonNullable<Posting['interactions']>[number]['interactionType'];

export function withInteractions(...types: InteractionType[]): Posting {
  return makePosting({
    interactions: types.map((interactionType) => ({
      interactionType,
      jobId: 'post-1',
      timestamp: 0,
      title: 'T',
      company: 'C',
      url: 'u',
      source: 's',
    })),
  });
}

export const salaried = () =>
  makePosting({ salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'EUR' });

export function setup(posting: Posting = makePosting()) {
  return renderHook(() => usePostingActions(posting)).result;
}

export type Actions = ReturnType<typeof usePostingActions>;
type Result = ReturnType<typeof setup>;

/** Run `call` against the hook's current actions inside `act`, awaiting its promise. */
export async function run(result: Result, call: (a: Actions) => unknown) {
  await act(async () => {
    await call(result.current);
  });
}

export const withMessage = (message: string) => expect.objectContaining({ message });

/** Call from `beforeEach`. */
export function resetActions() {
  mockOpenExternalAsync.mockClear();
  mockPersistJobAsync.mockClear();
  mockSaveFromPostingAsync.mockClear();
  mockSaveFromPostingAsync.mockResolvedValue({ id: 'app-1' });
  mockNavigate.mockClear();
  mockSetApplicationApply.mockClear();
  notifySuccess.mockClear();
  notifyError.mockClear();
  saveState.isPending = false;
  mockUseRowMatchScore.mockReturnValue(noScore);
}
