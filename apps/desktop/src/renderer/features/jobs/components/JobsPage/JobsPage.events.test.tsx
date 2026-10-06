/**
 * JobsPage — partial-failure scrape summary.
 *
 * Covers the job.completed / job.failed event handler in JobsPage/index.tsx:
 *   - A completed event with failed boards produces ok:true (not ok:false)
 *   - The partial note uses display names via t('jobs.boards.<id>'), not raw ids
 *   - The note follows the "N of M · <names> failed" format
 *   - A completed event with no failed boards produces no note (ok:true, note undefined)
 *   - All boards failing still produces ok:true (the event type is 'job.completed')
 *   - job.failed event → ok:false
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { waitFor } from '@testing-library/react';

import { fireJobEvent, jobEvents } from './job-events';
import { notifyMock, renderJobsPage, resetPage, scrapingMock } from './page-harness';

beforeEach(resetPage);

/** Fire a job.completed for the active job and return what the page reported. */
async function completeWith(boards: unknown) {
  renderJobsPage();
  fireJobEvent({ type: 'job.completed', jobId: 'job-123', data: { boards } });
  return finishedOutcome();
}

async function finishedOutcome() {
  await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
  return scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
}

describe('JobsPage — job.completed event handler', () => {
  it('registers a job events listener on mount', () => {
    renderJobsPage();
    expect(jobEvents.handler).toBeTypeOf('function');
  });

  it('completed event with no failed boards → ok:true, no note', async () => {
    const outcome = await completeWith([
      { board: 'linkedin', count: 10 },
      { board: 'indeed', count: 5 },
    ]);
    expect(outcome?.ok).toBe(true);
    expect(outcome?.note).toBeUndefined();
  });

  it('completed event with one failed board → ok:true (partial failure keeps ok)', async () => {
    const outcome = await completeWith([
      { board: 'linkedin', count: 10 },
      { board: 'indeed', count: 0, error: 'rate limited' },
    ]);
    expect(outcome?.ok).toBe(true);
    expect(outcome?.note).toBeDefined();
  });

  it('partial note uses translated display names via t("jobs.boards.<id>"), not raw ids', async () => {
    // t() mock format: "key[param=value,...]" so we can see exactly what was passed.
    const outcome = await completeWith([
      { board: 'linkedin', count: 5 },
      { board: 'indeed', count: 0, error: 'blocked' },
    ]);
    const note = outcome?.note ?? '';

    // The note is the result of t('jobs.partialScrapeNote', { done, total, failed }).
    // Our mock returns "jobs.partialScrapeNote[done=1,total=2,failed=<failedNames>]".
    // <failedNames> = t('jobs.boards.indeed') = 'jobs.boards.indeed' (not raw 'indeed').
    expect(note).toContain('jobs.partialScrapeNote');
    // The failed param must contain the translated key 'jobs.boards.indeed', not the raw id
    expect(note).toContain('jobs.boards.indeed');
    // Raw board id alone must not appear as the label
    expect(note).not.toMatch(/failed=indeed[,\]]/);
  });

  it('partial note format — N of M counts are correct', async () => {
    // Two boards, one fails → done=1, total=2
    const outcome = await completeWith([
      { board: 'greenhouse', count: 8 },
      { board: 'xing', count: 0, error: 'login required' },
    ]);
    const note = outcome?.note ?? '';

    // t() mock → "jobs.partialScrapeNote[done=1,total=2,failed=jobs.boards.xing]"
    expect(note).toContain('done=1'); // 1 board succeeded
    expect(note).toContain('total=2'); // 2 total boards
    expect(note).toContain('jobs.boards.xing'); // display name for the failed board
  });

  it('all boards failing → still ok:true (job.completed event type)', async () => {
    const outcome = await completeWith([
      { board: 'linkedin', count: 0, error: 'blocked' },
      { board: 'indeed', count: 0, error: 'rate limited' },
    ]);
    expect(outcome?.ok).toBe(true);
  });

  it('job.failed event → ok:false with the SANITIZED error data as note', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.failed',
      jobId: 'job-123',
      data: 'connection refused',
    });

    const outcome = await finishedOutcome();
    expect(outcome?.ok).toBe(false);
    // Security advisory: the form-footer note is sanitized too, not the raw error.
    expect(outcome?.note).toBe('sanitized:connection refused');
  });

  it('a partial-failure completion still keeps the scrapeOutcome note for the form footer', async () => {
    const outcome = await completeWith([
      { board: 'linkedin', count: 5 },
      { board: 'xing', count: 0, error: 'rate limited' },
    ]);
    expect(outcome?.ok).toBe(true);
    expect(outcome?.note).toContain('jobs.boards.xing');
    // The skip-toast path is gone entirely.
    expect(notifyMock.warning).not.toHaveBeenCalled();
  });

  it('malformed data.boards (not an array) → does not throw, noteScrapeFinished called with ok:true and no note', async () => {
    const outcome = await completeWith({});
    expect(outcome?.ok).toBe(true);
    expect(outcome?.note).toBeUndefined();
  });
});
