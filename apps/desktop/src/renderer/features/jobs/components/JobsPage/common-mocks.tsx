/**
 * Harness shared by every JobsPage suite: the stubs they all agree on, plus the
 * `useJobEvents` capture so a suite can fire synthetic backend events.
 *
 * This module must load BEFORE `./index` so its mocks apply — keep the name
 * sorting ahead of `index` (the import-sort rule orders relative imports
 * alphabetically). Each suite also declares its OWN `@/services`, `@ajh/ui`, `@ajh/translations` and child
 * component stubs — those are where the suites genuinely differ.
 */
import type { ReactNode } from 'react';
import { vi } from 'vitest';
import { act } from '@testing-library/react';

vi.mock('@/features/jobs/hooks/usePostingsSearch', () => ({
  usePostingsSearch: () => ({
    state: 'idle',
    result: null,
    committedQuery: '',
    search: vi.fn(),
    retry: vi.fn(),
    clear: vi.fn(),
    enableSemanticRanking: vi.fn(),
  }),
}));

vi.mock('@/hooks/useDefaultResumeId', () => ({ useDefaultResumeId: () => null }));

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => (ts: number) => String(ts),
}));

vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('@/features/jobs/providers', () => ({
  MatchScoresProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

/** Set by the suite's `useJobEvents` mock when the page subscribes. */
export const jobEvents = { handler: null as ((event: unknown) => void) | null };

export function fireJobEvent(event: unknown) {
  act(() => {
    jobEvents.handler?.(event);
  });
}
