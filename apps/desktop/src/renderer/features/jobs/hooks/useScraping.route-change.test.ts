/**
 * useScraping — the scrape survives a route change (AGENTS.md rule 16).
 *
 * A route change unmounts the page (and this hook) while the Rust scrape keeps
 * running; each defect below got a test that fails if the field goes back to
 * component state. Fake timers drive the watchdog that re-arms off the stored
 * job id.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { useSessionStore } from '@/store/session-store';

import {
  cancelMutateAsync,
  fetchJobMock,
  flushLeadingPoll,
  invalidatePostingsMock,
  makeForm,
  mount,
  mountAndStart,
  mutateAsync,
  resetScraping,
  sentPayload,
  sentReplace,
  settleWatchdog,
  start,
} from './scraping-harness';

beforeEach(resetScraping);
afterEach(() => {
  vi.useRealTimers();
});

// Defect 1 — DATA LOSS: "Show more" after a route change must not send
// replace:true (which clears the persisted postings cache on the first item).
describe('useScraping — the scrape survives a route change', () => {
  it('"Show more" after an unmount/remount APPENDS (no replace flag on the wire)', async () => {
    const form = makeForm({ query: 'engineer', amount: 25 });

    // 1. The user runs a search on the jobs page.
    const first = await mountAndStart(form);
    expect(sentReplace(0)).toBe(true);

    // 2. The user navigates to Settings — the page (and this hook) unmounts.
    first.unmount();

    // 3. …and comes back. Fresh component state, same session store.
    const second = mount(form);

    // 4. "Show more": same criteria, a bigger amount. Contract: APPEND.
    await start(second, 50);

    expect(mutateAsync).toHaveBeenCalledTimes(2);
    expect(sentReplace(1)).toBeUndefined();
    expect(sentPayload(1)).toMatchObject({ query: 'engineer', amount: 50 });
  });

  it('a genuinely different search after a remount still REPLACES', async () => {
    const first = await mountAndStart(makeForm({ query: 'rust' }));
    first.unmount();

    await mountAndStart(makeForm({ query: 'python' }));

    expect(sentReplace(1)).toBe(true);
  });
});

// Defect 2 — the in-flight scrape must stay cancellable across a route change.
describe('useScraping — the in-flight job survives a route change', () => {
  it('remounts into `scraping` and cancels the job the PREVIOUS mount started', async () => {
    const form = makeForm();

    const first = await mountAndStart(form);
    expect(first.result.current.scraping).toBe(true);
    first.unmount();

    const second = mount(form);

    // Both the progress strip and the Cancel button are gated on `scraping`.
    expect(second.result.current.scraping).toBe(true);
    expect(second.result.current.scrapeJobId).toBe('j1');

    await act(async () => {
      await second.result.current.cancelScrape();
    });

    expect(cancelMutateAsync).toHaveBeenCalledWith('j1');
    expect(second.result.current.scraping).toBe(false);
    expect(second.result.current.scrapeJobId).toBeNull();
  });

  it('the next search after a remount cancels the orphaned scrape first', async () => {
    const first = await mountAndStart(makeForm());
    first.unmount();

    // A second scrape from a fresh mount: without the stored job id this used
    // to start a rival run writing into the same postings cache.
    await mountAndStart(makeForm({ query: 'another' }));

    expect(cancelMutateAsync).toHaveBeenCalledWith('j1');
    expect(cancelMutateAsync).toHaveBeenCalledTimes(1);
  });

  it('a remount onto a job that already FINISHED settles to a finished state', async () => {
    // Still running while `first` is mounted — the leading watchdog poll (see
    // the progress tests below) must not settle it before the page ever
    // navigates away, or this test would stop exercising the remount path.
    fetchJobMock.mockResolvedValue({ status: 'running' });
    vi.useFakeTimers();
    const form = makeForm();
    const first = await mountAndStart(form);
    first.unmount();

    // The job finishes only now, while nothing is mounted to hear the event.
    fetchJobMock.mockResolvedValue({ status: 'completed', result: { boards: [] } });

    // The job.completed EVENT is never delivered — the subscription lives on
    // the unmounted page. Only the watchdog can reconcile this.
    const second = mount(form);
    expect(second.result.current.scraping).toBe(true);

    await settleWatchdog();

    expect(second.result.current.scraping).toBe(false);
    expect(second.result.current.scrapeJobId).toBeNull();
    expect(second.result.current.scrapeOutcome).toEqual({ ok: true });
  });
});

// Defect 3 — the per-board diagnostics ("aggregator: 429 rate limited") are the
// only explanation of an empty result; they must survive navigation AND a
// scrape that finishes while the user is on another route.
describe('useScraping — per-board diagnostics survive', () => {
  it('recovers the per-board summaries from the job tracker after finishing off-page', async () => {
    const boards = [{ board: 'aggregator', count: 0, error: '429 rate limited' }];
    fetchJobMock.mockResolvedValue({ status: 'completed', result: { count: 0, boards } });
    vi.useFakeTimers();
    const form = makeForm();
    const first = await mountAndStart(form);
    first.unmount();

    mount(form);
    await settleWatchdog();

    expect(useSessionStore.getState().jobs.scrapeSummaries).toEqual(boards);
  });
});

// Defect 4 — a remount mid-scrape must not report a false 0% for the rest of
// the run. `useScrapeProgress` (mocked to `null`, matching the real hook
// resetting on every mount) never fires again on the single-board default, so
// the persisted `JobRecord.progress` the watchdog already polls is the only
// surviving source of truth.
describe('useScraping — progress survives a route change', () => {
  it('reports the backend-persisted fraction immediately after a remount', async () => {
    fetchJobMock.mockResolvedValue({ status: 'running', progress: 0.8 });
    const form = makeForm();

    const first = await mountAndStart(form);
    first.unmount();

    const second = mount(form);
    // The leading (non-timer) poll must resolve before this assertion.
    await flushLeadingPoll();

    expect(second.result.current.scrapeProgress).toBe(0.8);
  });

  it('does not fabricate progress when the backend has none yet', async () => {
    fetchJobMock.mockResolvedValue({ status: 'running' });
    const form = makeForm();

    const first = await mountAndStart(form);
    first.unmount();

    const second = mount(form);
    await flushLeadingPoll();

    expect(second.result.current.scrapeProgress).toBeNull();
  });
});

// Defect 5 — a scrape that fails while the user is on another route must come
// back with an explanation (chip strip + note), not silently to an empty
// strip — and the backend error must be sanitized the same way the live
// `job.failed` event path already is (path-privacy: AGENTS.md).
describe('useScraping — a failed scrape explains itself after a route change', () => {
  it('restores scrapeSummaries/scrapeFailureNote, sanitized, after failing off-page', async () => {
    fetchJobMock.mockResolvedValue({ status: 'running' });
    const form = makeForm();

    const first = await mountAndStart(form);
    first.unmount();

    // The job fails only now, while nothing is mounted to hear the event —
    // and the backend error carries a local path, same as a real filesystem
    // failure would.
    fetchJobMock.mockResolvedValue({
      status: 'failed',
      error: 'failed to read C:\\Users\\alice\\creds.json',
    });
    vi.useFakeTimers();
    const second = mount(form);
    await settleWatchdog();

    expect(second.result.current.scraping).toBe(false);
    const { scrapeSummaries, scrapeFailureNote, scrapeOutcome } = useSessionStore.getState().jobs;
    // An empty (not stale/undefined) strip — the caller renders it whenever
    // the array is non-null, so `[]` is what makes the note actually show.
    expect(scrapeSummaries).toEqual([]);
    expect(scrapeFailureNote).toContain('failed to read');
    expect(scrapeFailureNote).toContain('<path-redacted>');
    expect(scrapeFailureNote).not.toMatch(/alice/i);
    // The drawer's own outcome note (`ScrapeForm`) must be the SAME sanitized
    // text, not the raw backend string — reopening "New Scrape" later must
    // never show the path either.
    expect(scrapeOutcome?.note).toBe(scrapeFailureNote);
    expect(scrapeOutcome?.note).not.toMatch(/alice/i);
  });
});

// Defect 6 — the watchdog's own completion recovery must invalidate the
// postings cache exactly like the live `job.completed` event handler does, or
// a 30s `staleTime` leaves the pre-scrape (possibly empty) list on screen.
describe('useScraping — completing off-page still refreshes the postings cache', () => {
  it('invalidates postings after the watchdog recovers a completed scrape', async () => {
    fetchJobMock.mockResolvedValue({ status: 'running' });
    const form = makeForm();

    const first = await mountAndStart(form);
    first.unmount();

    fetchJobMock.mockResolvedValue({ status: 'completed', result: { boards: [] } });
    vi.useFakeTimers();
    mount(form);
    await settleWatchdog();

    expect(invalidatePostingsMock).toHaveBeenCalled();
  });
});
