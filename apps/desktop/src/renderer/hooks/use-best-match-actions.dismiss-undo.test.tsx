/**
 * useBestMatchActions — Dismiss / Undo, wired against a REAL QueryClient +
 * mock AppClient (not a mocked `@/services`), so the actual cache-timing bug
 * is reachable: a premature `invalidateQueries` on `keys.autopilot.all`
 * forces `useBestMatches` to refetch before the user could ever click Undo,
 * and the backend mock (like the real `compute_best_matches`) excludes a
 * dismissed job from that refetch — evicting the row from the cache
 * entirely and making Undo a dead button. The earlier unit test only
 * asserted `dismissedKeys` gained/lost a key, which is a test of a `useState`
 * setter: it stayed green with the feature completely broken. This one
 * renders the row, dismisses it, asserts it is gone, clicks Undo, and
 * asserts the row is VISIBLE AGAIN.
 */

import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { Autopilot, AutopilotBestMatch, AutopilotBestMatchesResult } from '@ajh/shared';

import { createMockClient } from '@/test-support';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@ajh/ui', () => ({
  useNotification: () => ({ success: vi.fn(), error: vi.fn() }),
}));

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
}));

import { AppClientProvider } from '@/providers/AppClientProvider';
import { useBestMatches } from '@/services';

import { useBestMatchActions } from './use-best-match-actions';

function makeMatch(): AutopilotBestMatch {
  return {
    key: 'k1',
    title: 'Staff Backend Engineer',
    company: 'Acme',
    url: 'https://example.com/job/1',
    location: 'Berlin',
    score: 80,
    scoreSource: 'combined',
    foundAt: 0,
    sources: [{ autopilotId: 'ap-1', autopilotName: 'Berlin roles', paused: false, foundAt: 0 }],
  };
}

/** Minimal stand-in for BestMatchRow/DismissedBestMatchRow — real enough to
 *  exercise the exact hook interaction under test without pulling in @ajh/ui
 *  primitives, routing, or icons unrelated to the bug. */
function Harness() {
  const { data } = useBestMatches();
  const { dismissedKeys, handleDismiss, undoDismiss } = useBestMatchActions();
  const matches = data?.matches ?? [];
  return (
    <div>
      {matches.map((m) =>
        dismissedKeys.has(m.key) ? (
          <div key={m.key}>
            <span>row-dismissed</span>
            <button type="button" onClick={() => undoDismiss(m.key)}>
              Undo
            </button>
          </div>
        ) : (
          <div key={m.key}>
            <span>{m.title}</span>
            <button type="button" onClick={() => handleDismiss(m)}>
              Dismiss
            </button>
          </div>
        )
      )}
    </div>
  );
}

/** Builds a mock AppClient whose `autopilot.bestMatches()` mirrors the real
 *  `compute_best_matches`: once a job's been dismissed (its persisted
 *  interaction recorded), a later fetch excludes it. This is what makes the
 *  regression actually visible rather than just asserting call counts. */
function makeWiredClient() {
  const dismissedOnBackend = new Set<string>();
  const match = makeMatch();

  const bestMatches = vi.fn(async (): Promise<AutopilotBestMatchesResult> =>
    dismissedOnBackend.has(match.url)
      ? { matches: [], total: 0, autopilotCount: 0 }
      : { matches: [match], total: 1, autopilotCount: 1 }
  );

  const persistJob = vi.fn(async (req: { job: { url: string }; interactionType: string }) => {
    if (req.interactionType === 'dismissed') dismissedOnBackend.add(req.job.url);
  });

  const client = createMockClient({
    'autopilot.bestMatches': bestMatches,
    'autopilot.list': async (): Promise<Autopilot[]> => [],
    'scrape.persistJob': persistJob,
  });

  return { client, bestMatches, persistJob, match };
}

function renderHarness(client: ReturnType<typeof createMockClient>) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: Infinity, staleTime: Infinity },
      mutations: { retry: false },
    },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <AppClientProvider client={client}>{children}</AppClientProvider>
    </QueryClientProvider>
  );
  return { ...render(<Harness />, { wrapper }), queryClient };
}

describe('useBestMatchActions — Dismiss then Undo (real QueryClient)', () => {
  it('hides the row on Dismiss, then Undo brings it back — no premature refetch evicts it', async () => {
    const user = userEvent.setup();
    const { client, bestMatches, persistJob, match } = makeWiredClient();
    renderHarness(client);

    await waitFor(() => expect(screen.getByText(match.title)).toBeInTheDocument());
    expect(bestMatches).toHaveBeenCalledTimes(1);

    await user.click(screen.getByText('Dismiss'));

    // Optimistic hide fires immediately.
    expect(screen.queryByText(match.title)).not.toBeInTheDocument();
    expect(screen.getByText('row-dismissed')).toBeInTheDocument();

    // Let the persistJob mutation resolve.
    await waitFor(() => expect(persistJob).toHaveBeenCalledTimes(1));

    await user.click(screen.getByText('Undo'));

    // The whole point of Undo: the row must come back.
    await waitFor(() => expect(screen.getByText(match.title)).toBeInTheDocument());
    expect(screen.queryByText('row-dismissed')).not.toBeInTheDocument();
  });
});
