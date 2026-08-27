/**
 * useBestMatchActions — Dismiss / Undo, wired against a REAL QueryClient +
 * mock AppClient (not a mocked `@/services`), so the actual cache-timing bug
 * is reachable: a premature `invalidateQueries` on `keys.autopilot.all`
 * forces `useBestMatches` to refetch before the user could ever click Undo,
 * and the backend mock (like the real `compute_best_matches`) excludes a
 * dismissed job from that refetch — evicting the row from the cache
 * entirely and making Undo a dead button. The earlier unit test only
 * asserted `dismissedKeys` gained/lost a key, which is a test of a `useState`
 * setter: it stayed green with the feature completely broken.
 *
 * Undo is now a REAL server-side undo (`scrape.removeInteraction`), not just
 * a local reveal — so the mock backend here mirrors that too: it tracks
 * dismissals in the SAME set `persistJob`/`removeInteraction` both mutate,
 * and `bestMatches()` excludes exactly what's currently in that set. This
 * test renders the row, dismisses it, asserts it is gone, clicks Undo, and
 * — the assertion the earlier optimistic-only version could NOT make —
 * forces a REFETCH after Undo and asserts the row is STILL visible, proving
 * the persisted dismissal was actually deleted, not just hidden client-side.
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
  const { data, refetch } = useBestMatches();
  const { dismissedKeys, handleDismiss, undoDismiss } = useBestMatchActions();
  const matches = data?.matches ?? [];
  return (
    <div>
      <button type="button" onClick={() => void refetch()}>
        Refetch
      </button>
      {matches.map((m) =>
        dismissedKeys.has(m.key) ? (
          <div key={m.key}>
            <span>row-dismissed</span>
            <button type="button" onClick={() => undoDismiss(m.key, m.url)}>
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
 *  interaction recorded), a later fetch excludes it. `removeInteraction`
 *  deletes from the SAME set `persistJob` writes into — mirroring the real
 *  `InteractionStore::remove`/`upsert` sharing one on-disk record — so a
 *  refetch after a successful Undo can see the job again. */
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

  const removeInteraction = vi.fn(async (req: { jobId: string; interactionType: string }) => {
    if (req.interactionType !== 'dismissed') return false;
    return dismissedOnBackend.delete(req.jobId);
  });

  const client = createMockClient({
    'autopilot.bestMatches': bestMatches,
    'autopilot.list': async (): Promise<Autopilot[]> => [],
    'scrape.persistJob': persistJob,
    'scrape.removeInteraction': removeInteraction,
  });

  return { client, bestMatches, persistJob, removeInteraction, match };
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
  it('hides the row on Dismiss, then Undo brings it back — and STAYS back after a refetch', async () => {
    const user = userEvent.setup();
    const { client, bestMatches, persistJob, removeInteraction, match } = makeWiredClient();
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

    // The optimistic reveal fires immediately.
    await waitFor(() => expect(screen.getByText(match.title)).toBeInTheDocument());
    expect(screen.queryByText('row-dismissed')).not.toBeInTheDocument();

    // The real assertion this test exists for: the persisted dismissal must
    // actually be GONE server-side, not just hidden client-side. Wait for the
    // removal to land, then force a FRESH fetch (independent of the
    // onSuccess-triggered one) and confirm the row survives it — this is what
    // an optimistic-only Undo (the earlier, dead-button version) could not do:
    // any refetch after that version re-excluded the job forever.
    await waitFor(() => expect(removeInteraction).toHaveBeenCalledTimes(1));
    expect(removeInteraction).toHaveBeenCalledWith({
      jobId: match.url,
      interactionType: 'dismissed',
    });

    const bestMatchesCallsBeforeRefetch = bestMatches.mock.calls.length;
    await user.click(screen.getByText('Refetch'));

    await waitFor(() =>
      expect(bestMatches.mock.calls.length).toBeGreaterThan(bestMatchesCallsBeforeRefetch)
    );
    expect(screen.getByText(match.title)).toBeInTheDocument();
    expect(screen.queryByText('row-dismissed')).not.toBeInTheDocument();
  });
});
