/**
 * JobsResults — selectedId reconciliation (unified selection effect).
 *
 * The unified effect (deps: waiting, viewMode, topId, selectionInDisplay, setJobs)
 * handles both reconciliation and split-mode auto-select:
 *
 *   • List mode: effect is a no-op regardless of selection validity — list mode
 *     has no auto-select requirement and selectedId is not consumed by any list
 *     mode UI.
 *   • Split mode + stale/absent selection: selects display[0] (not null) so the
 *     detail pane is never left blank.
 *   • Split mode + valid selection still in display: no-op (no clobber).
 *   • Empty display (topId === null): effect early-returns; EmptyState handles UI.
 *
 * Strategy:
 *  - Uses the same session-store mock shape as JobsResults.test.tsx.
 *  - JobsSplitView is stubbed (so split-mode renders cheaply).
 *  - PostingRow is stubbed (no router/provider deps needed).
 *  - Virtualizer is stubbed to render all items synchronously.
 *  - MatchScoresProvider is stubbed via useJobMatchScore (no AI deps).
 */

import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── router ────────────────────────────────────────────────────────────────────

vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: vi.fn() }),
}));

// ── session-store — MUTABLE so tests can change selectedId between renders ────

const mockSetJobs = vi.fn();
const mockSetSettings = vi.fn();

// Mutable object: tests mutate STORE_STATE.jobs before each render.
const STORE_STATE = {
  setSettings: mockSetSettings,
  jobs: { viewMode: 'list' as 'list' | 'split', selectedId: null as string | null },
  setJobs: mockSetJobs,
};

vi.mock('@/store/session-store', () => ({
  useSessionStore: (sel?: (s: typeof STORE_STATE) => unknown) =>
    sel ? sel(STORE_STATE) : STORE_STATE,
}));

// ── useHasProviderKey ─────────────────────────────────────────────────────────

vi.mock('@/services/use-ai-provider', () => ({
  useHasProviderKey: (_provider: string, enabled = true) => ({
    data: enabled ? { has: true } : undefined,
    isSuccess: true,
  }),
}));

// ── MatchScoresProvider dependency — provider calls useJobMatchScore per row ──

vi.mock('@/services', () => ({
  useJobMatchScore: () => ({ data: undefined }),
}));

// ── PostingRow stub ───────────────────────────────────────────────────────────

vi.mock('@/features/jobs/components/PostingRow', () => ({
  PostingRow: ({ posting }: { posting: { id: string; title: string } }) => (
    <div data-testid={TEST_IDS.jobs.postingRow} data-id={posting.id}>
      {posting.title}
    </div>
  ),
}));

// ── JobsSplitView stub ────────────────────────────────────────────────────────

vi.mock('@/features/jobs/components/JobsSplitView', () => ({
  JobsSplitView: ({ display }: { display: { id: string }[] }) => (
    <div data-testid="split-view">{display.map((p) => p.id).join(',')}</div>
  ),
}));

// ── Virtualizer stub — render all items synchronously ────────────────────────

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 88,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        key: index,
        index,
        start: index * 88,
      })),
    measureElement: () => {},
  }),
}));

// ── MatchScoresProvider ───────────────────────────────────────────────────────

import { MatchScoresProvider } from '@/features/jobs/providers';
import type { Posting } from '@/features/jobs/types';

import { JobsResults } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function posting(id: string): Posting {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/${id}`,
    title: id,
    company: 'Acme',
    description: '',
    capturedAt: 0,
  };
}

const noop = () => {};
const formatRelativeTime = () => '';

function renderResults(
  filtered: Posting[],
  resumeId: string | null = null,
  absorbedInto?: Map<string, string>
) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <MatchScoresProvider resumeId={resumeId}>{children}</MatchScoresProvider>
  );
  return render(
    <JobsResults
      filtered={filtered}
      formatRelativeTime={formatRelativeTime}
      scraping={false}
      absorbedInto={absorbedInto}
      onShowMore={noop}
      onScrape={noop}
    />,
    { wrapper }
  );
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockSetJobs.mockClear();
  mockSetSettings.mockClear();
  STORE_STATE.jobs = { viewMode: 'list', selectedId: null };
});

// ─────────────────────────────────────────────────────────────────────────────
// Reconciliation — selectedId removed from display
// ─────────────────────────────────────────────────────────────────────────────

describe('JobsResults — selectedId reconciliation', () => {
  // ── List mode — effect is a no-op ─────────────────────────────────────────

  it('does NOT call setJobs in list mode even when selectedId is stale', async () => {
    // List mode: the unified effect only acts in split mode.
    STORE_STATE.jobs = { viewMode: 'list', selectedId: 'job-gone' };

    await act(async () => {
      renderResults([posting('job-a'), posting('job-b')]);
    });

    expect(mockSetJobs).not.toHaveBeenCalled();
  });

  it('does NOT call setJobs in list mode when selectedId is still present', async () => {
    STORE_STATE.jobs = { viewMode: 'list', selectedId: 'job-a' };

    await act(async () => {
      renderResults([posting('job-a'), posting('job-b')]);
    });

    expect(mockSetJobs).not.toHaveBeenCalled();
  });

  it('does NOT call setJobs in list mode when selectedId is null', async () => {
    STORE_STATE.jobs = { viewMode: 'list', selectedId: null };

    await act(async () => {
      renderResults([posting('job-a')]);
    });

    expect(mockSetJobs).not.toHaveBeenCalled();
  });

  it('does NOT call setJobs when display is empty (topId guard — EmptyState handles UI)', async () => {
    // Both list and split mode: topId === null → effect early-returns.
    STORE_STATE.jobs = { viewMode: 'list', selectedId: 'job-x' };

    await act(async () => {
      renderResults([]); // empty display → EmptyState shown, effect no-ops
    });

    expect(mockSetJobs).not.toHaveBeenCalled();
  });

  // ── Split mode — selects display[0] when selection is stale/absent ────────

  it('selects display[0] in split mode when selectedId is no longer in display', async () => {
    // Old reconciliation nulled selectedId; new behaviour selects the new top
    // so the detail pane is never left blank.
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'job-missing' };

    await act(async () => {
      renderResults([posting('job-present')]);
    });

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'job-present' });
  });

  it('does NOT call setJobs in split mode when selectedId is still present', async () => {
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'job-a' };

    await act(async () => {
      renderResults([posting('job-a'), posting('job-b')]);
    });

    expect(mockSetJobs).not.toHaveBeenCalled();
  });

  // ── Regression: a selected id absorbed by mergePostings' cross-source collapse
  // must re-point at the survivor, NOT silently fall back to display[0] ─────────
  //
  // Root-cause scenario: boards=[aggregator, board]. The fast `board` streams
  // first, so its live-stream row (id `board-1`) is the only copy and gets
  // selected by the user. `livePostings` is never cleared on job.completed. The
  // persisted refetch holds the SAME job under `aggregator-1` (the engine's
  // incumbent-selection order follows board input order, not display arrival
  // order). mergePostings' pass 2 absorbs `board-1` into `aggregator-1` — before
  // this fix, `selectedId` (`board-1`) vanished from `filtered` and the existing
  // reconciliation effect silently swapped the detail pane to display[0], an
  // UNRELATED job.
  it('re-points selection at the survivor id when the selected live-id row was absorbed by a persisted incumbent (not display[0])', async () => {
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'board-1' };
    const absorbedInto = new Map([['board-1', 'aggregator-1']]);

    await act(async () => {
      // `board-1` no longer appears — mergePostings already collapsed it into
      // `aggregator-1`, which is unrelated-job-shaped ('unrelated-job') to prove
      // the fix isn't accidentally passing via display[0] coincidentally matching.
      renderResults([posting('unrelated-job'), posting('aggregator-1')], null, absorbedInto);
    });

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'aggregator-1' });
    expect(mockSetJobs).not.toHaveBeenCalledWith({ selectedId: 'unrelated-job' });
  });

  it('falls back to display[0] when the selected id was absorbed but the survivor is somehow not in display', async () => {
    // Defensive: absorbedInto claims a survivor that isn't actually in `filtered`
    // (should not happen in practice — the survivor is always in the merged
    // output — but the effect must not select a phantom id).
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'board-1' };
    const absorbedInto = new Map([['board-1', 'not-in-display']]);

    await act(async () => {
      renderResults([posting('job-present')], null, absorbedInto);
    });

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'job-present' });
  });

  it('does NOT call setJobs when the selection is absorbed but the caller passes no absorbedInto map', async () => {
    // absorbedInto omitted entirely (e.g. a caller that never merges duplicate
    // sources) — must fall back to the existing topId behaviour, not throw.
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'job-missing' };

    await act(async () => {
      renderResults([posting('job-present')]);
    });

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'job-present' });
  });
});
