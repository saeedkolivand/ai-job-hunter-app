/**
 * JobsPage — per-board chip strip + outright-failure note: retention, wiring,
 * active-job guard, and mutual exclusivity with the empty state.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { screen, waitFor } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import { fireJobEvent } from './job-events';
import {
  boardChips,
  notifyMock,
  postingsContainer,
  renderJobsPage,
  resetPage,
  resultsProps,
  samplePosting,
  scrapingMock,
} from './page-harness';

beforeEach(resetPage);

const completed = (boards: unknown, jobId = 'job-123') =>
  fireJobEvent({ type: 'job.completed', jobId, data: { boards } });
const failed = (data: string, jobId = 'job-123') =>
  fireJobEvent({ type: 'job.failed', jobId, data });

describe('JobsPage — per-board chip strip retention (replaces the old skip-toasts)', () => {
  it('retains the full per-board summaries and feeds them to the chip strip', async () => {
    // A partial-failure completion implies results ARE present (linkedin
    // returned 5) — set postings so the header-strip results-present gate is
    // satisfied and the retention can be observed at the header too.
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    const boards = [
      { board: 'linkedin', count: 5 },
      { board: 'indeed', count: 0, skipped: 'needs-login' },
      { board: 'xing', count: 0, error: 'rate limited' },
    ];
    completed(boards);

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    // The strip receives the untouched summaries (counts + skip + error), not a
    // lossy names-only projection that discards the "why".
    expect(boardChips.summaries).toEqual(boards);
    // The same data reaches the empty-state wiring in JobsResults.
    expect(resultsProps.boardSummaries).toEqual(boards);
  });

  it('surfaces a skipped board via the strip, NOT a toast (toasts were folded in)', async () => {
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    completed([{ board: 'indeed', count: 0, skipped: 'needs-login' }]);

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    expect(boardChips.summaries).toEqual([{ board: 'indeed', count: 0, skipped: 'needs-login' }]);
    // No transient warning toast — the strip is the persistent surface now.
    expect(notifyMock.warning).not.toHaveBeenCalled();
  });

  it('needs-keys and needs-company skips also route to the strip, no toast', async () => {
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    const boards = [
      { board: 'aggregator', count: 0, skipped: 'needs-keys' },
      { board: 'greenhouse', count: 0, skipped: 'needs-company' },
    ];
    completed(boards);

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    expect(boardChips.summaries).toEqual(boards);
    expect(notifyMock.warning).not.toHaveBeenCalled();
  });

  it('a stale (inactive-job) completion does NOT overwrite the strip', async () => {
    // Postings present so the header WOULD render if the active-job guard were
    // broken — isolates this test from the separate results-present gate.
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    // The active scrape is 'job-123' (seeded by resetPage); fire a DIFFERENT id.
    completed([{ board: 'indeed', count: 0, skipped: 'needs-login' }], 'other-job');

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    // The active-job guard returns before the summaries are stored, so the
    // header strip never rendered — its capture stays at the reset default.
    expect(boardChips.summaries).toBeNull();
  });

  it('job.failed clears the retained summaries (an outright failure has no per-board data)', async () => {
    renderJobsPage();

    // A completed run first populates the retained summaries (asserted via the
    // unconditional resultsProps signal — decoupled from header visibility)...
    completed([{ board: 'linkedin', count: 3 }]);
    await waitFor(() =>
      expect(resultsProps.boardSummaries).toEqual([{ board: 'linkedin', count: 3 }])
    );

    // ...then an outright failure clears it (surfaced via scrapeOutcome instead).
    failed('connection refused');
    await waitFor(() => expect(resultsProps.boardSummaries).toEqual([]));
  });

  it('a stale (inactive-job) job.failed does NOT wipe the strip or paint a foreign error (jobs:event is a shared global channel)', async () => {
    // Postings present so the header WOULD show a wipe/foreign-note if the
    // active-job guard were broken.
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    // First populate the strip via a real completion for the ACTIVE job.
    const boards = [{ board: 'linkedin', count: 5 }];
    completed(boards);
    await waitFor(() => expect(boardChips.summaries).toEqual(boards));

    // An unrelated background job (autopilot/AI/agent/pipeline — job.failed is
    // emitted on the SAME `jobs:event` channel) fails with a DIFFERENT jobId.
    failed('AI generation failed', 'unrelated-ai-job');

    // noteScrapeFinished still fires unconditionally (internally buffered/
    // guarded by job id — a foreign id is simply parked, never surfaced)...
    await waitFor(() =>
      expect(scrapingMock.noteScrapeFinished).toHaveBeenCalledWith('unrelated-ai-job', {
        ok: false,
        note: 'sanitized:AI generation failed',
      })
    );
    // ...but the strip and failure note are UNTOUCHED — no foreign wipe/paint.
    expect(boardChips.summaries).toEqual(boards);
    expect(resultsProps.boardSummaries).toEqual(boards);
    expect(resultsProps.failureNote).toBeNull();
    expect(screen.queryByText(/jobs\.lastScrapeFailed/)).not.toBeInTheDocument();
  });

  it('forwards the unfiltered posting count as totalCount (claude review advisory #2)', () => {
    postingsContainer.data = [samplePosting('a'), samplePosting('b'), samplePosting('c')];
    renderJobsPage();

    expect(resultsProps.totalCount).toBe(3);
  });
});

describe('JobsPage — outright failure note (no per-board summaries to chip)', () => {
  it('job.failed persists a SANITIZED failure note (not the raw error) for the empty state', async () => {
    // Zero results (default) — the note routes to JobsResults' empty state
    // (verified end-to-end in JobsResults.test.tsx); this test proves the
    // data-layer signal is sanitized before it ever leaves JobsPage. The
    // header's OWN rendering of this note (when results ARE present) is
    // covered by the mutual-exclusivity block below.
    renderJobsPage();

    failed('connection refused at C:\\Users\\me\\x');

    await waitFor(() =>
      expect(resultsProps.failureNote).toBe('sanitized:connection refused at C:\\Users\\me\\x')
    );
  });

  it('a subsequent job.completed clears the failure note', async () => {
    renderJobsPage();

    failed('boom');
    await waitFor(() => expect(resultsProps.failureNote).toBe('sanitized:boom'));

    completed([{ board: 'linkedin', count: 3 }]);
    await waitFor(() =>
      expect(resultsProps.boardSummaries).toEqual([{ board: 'linkedin', count: 3 }])
    );
    expect(resultsProps.failureNote).toBeNull();
  });
});

// The header chip strip + failure note must render ONLY alongside a visible
// results list — with zero results, JobsResults' empty state is the SOLE owner
// of the explanation (both used to co-render, duplicating the same message).
describe('JobsPage — header strip mutual exclusivity with the empty state', () => {
  it('ZERO results: the underlying data still reaches JobsResults, but the header strip does NOT render', async () => {
    renderJobsPage(); // postingsContainer.data = [] → filtered.length === 0

    completed([{ board: 'linkedin', count: 0, error: 'blocked' }]);

    await waitFor(() =>
      expect(resultsProps.boardSummaries).toEqual([
        { board: 'linkedin', count: 0, error: 'blocked' },
      ])
    );
    // Empty state owns it — the header's own strip instance never rendered.
    expect(boardChips.summaries).toBeNull();
    expect(screen.queryByTestId('board-summary-chips')).not.toBeInTheDocument();
  });

  it('ZERO results: the header failure note does NOT render (empty state owns it)', async () => {
    renderJobsPage();

    failed('connection refused');

    await waitFor(() => expect(resultsProps.failureNote).toBe('sanitized:connection refused'));
    expect(screen.queryByText(/jobs\.lastScrapeFailed/)).not.toBeInTheDocument();
  });

  it('RESULTS PRESENT: the header renders the chip strip', async () => {
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    const boards = [
      { board: 'linkedin', count: 5 },
      { board: 'xing', count: 0, error: 'blocked' },
    ];
    completed(boards);

    await waitFor(() => expect(boardChips.summaries).toEqual(boards));
    expect(screen.getByTestId('board-summary-chips')).toBeInTheDocument();
  });

  it('RESULTS PRESENT: the header renders the failure note', async () => {
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    failed('connection refused');

    await waitFor(() => expect(resultsProps.failureNote).toBe('sanitized:connection refused'));
    // Deliberately TWO nodes: the visible (aria-hidden) chip-row copy and the
    // always-mounted sr-only live region the command bar announces through.
    expect(
      screen.getAllByText('jobs.lastScrapeFailed[reason=sanitized:connection refused]')
    ).toHaveLength(2);
    expect(screen.getByTestId(TEST_IDS.jobs.scrapeStatusLive)).toHaveTextContent(
      'jobs.lastScrapeFailed[reason=sanitized:connection refused]'
    );
  });
});
