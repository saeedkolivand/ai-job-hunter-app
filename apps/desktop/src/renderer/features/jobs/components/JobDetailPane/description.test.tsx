/**
 * JobDetailPane — EmptyState, markdown pass-through, on-demand description
 * resolve (aggregator gate, keep-longer merge, load button, error hint).
 *
 * noUncheckedIndexedAccess: array accesses guarded throughout.
 */

import { beforeEach, describe, expect, it } from 'vitest';
import { act, render, screen } from '@testing-library/react';

import {
  formatRelativeTime,
  JobDetailPane,
  makePosting,
  mockRefetch,
  mockTrackInteraction,
  mockUseResolveJobUrl,
  openPane,
  resetPaneMocks,
  resolveInFlight,
  resolveReturns,
  resolveSettled,
} from './harness';

beforeEach(resetPaneMocks);

describe('JobDetailPane — null posting', () => {
  it('renders EmptyState with jobs.selectAJob when posting is null', () => {
    render(<JobDetailPane posting={null} formatRelativeTime={formatRelativeTime} />);
    expect(screen.getByTestId('empty-state')).toHaveTextContent('jobs.selectAJob');
  });

  it('does not call trackInteraction when posting is null', () => {
    render(<JobDetailPane posting={null} formatRelativeTime={formatRelativeTime} />);
    expect(mockTrackInteraction).not.toHaveBeenCalled();
  });
});

describe('JobDetailPane — description rendering', () => {
  // JobDescription is from @ajh/ui (stub renders raw markdown string as text).
  // These tests verify the correct markdown string is passed through; the
  // actual GFM rendering is tested in @ajh/ui's own tests.

  it.each([
    ['plain', 'This is a great role.', 'This is a great role.'],
    ['bold', '**Strong skill** required.', '**Strong skill** required.'],
    ['heading', '## Requirements\n\nFive years of experience.', 'Requirements'],
  ])('passes %s markdown to JobDescription', async (id, description, expected) => {
    await openPane(makePosting(`md-${id}`, { description }));
    expect(screen.getByTestId('job-description')).toHaveTextContent(expected);
  });

  it('passes list markdown to JobDescription', async () => {
    await openPane(makePosting('md-list', { description: '- TypeScript\n- React\n- Rust' }));
    const el = screen.getByTestId('job-description');
    expect(el).toHaveTextContent('TypeScript');
    expect(el).toHaveTextContent('React');
    expect(el).toHaveTextContent('Rust');
  });

  it('passes link markdown to JobDescription (no live <a> in stub)', async () => {
    await openPane(makePosting('md-link', { description: '[Apply here](https://example.com)' }));
    expect(screen.getByTestId('job-description')).toHaveTextContent('Apply here');
    // Stub does not render an <a> — GFM link-as-span behavior tested in @ajh/ui
    expect(screen.queryByRole('link')).not.toBeInTheDocument();
  });
});

describe('JobDetailPane — useResolveJobUrl fallback', () => {
  it('shows loading text when description is empty and resolve is in-flight', async () => {
    resolveInFlight();
    await openPane(makePosting('job-load', { description: '' }));

    expect(screen.getByText('jobs.loadingDescription')).toBeInTheDocument();

    const statusEls = screen.getAllByRole('status');
    const busyEl = statusEls.find((el) => el.getAttribute('aria-busy') === 'true');
    expect(busyEl).toBeInTheDocument();
    expect(busyEl).toHaveAttribute('aria-busy', 'true');
  });

  it('shows fetched description when resolve data arrives and original description was empty', async () => {
    resolveSettled('Fetched job description text');
    await openPane(makePosting('job-fetched', { description: '' }));

    expect(screen.getByText('Fetched job description text')).toBeInTheDocument();
  });

  it('shows loading text when description is whitespace-only', async () => {
    resolveInFlight();
    await openPane(makePosting('job-ws', { description: '   ' }));

    expect(screen.getByText('jobs.loadingDescription')).toBeInTheDocument();
  });

  it('uses the posting description directly when description is non-empty on a non-aggregator source', async () => {
    await openPane(makePosting('job-has-desc', { description: 'Original description' }));

    expect(screen.getByText('Original description')).toBeInTheDocument();
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
  });
});

describe('JobDetailPane — aggregator short-description gate', () => {
  it('shows updating hint (not full loading state) when aggregator has a snippet and resolve is in-flight', async () => {
    // Existing snippet text is rendered immediately; a small "Updating…" hint
    // appears inline while the full text is being fetched. The full "Loading
    // description…" spinner is only shown when there is NO text.
    resolveInFlight();
    const posting = makePosting('agg-loading', {
      source: 'aggregator',
      description: 'Short Adzuna snippet.',
    });
    await openPane(posting);

    // Snippet is rendered immediately (no flash to full spinner)
    expect(screen.getByText('Short Adzuna snippet.')).toBeInTheDocument();
    // Inline updating hint shown while fetching
    expect(screen.getByText('jobs.updatingDescription')).toBeInTheDocument();
    // Full "loading" spinner NOT shown (that is reserved for when there is no text)
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
    expect(mockUseResolveJobUrl).toHaveBeenCalledWith(posting.url, true);
  });

  it('does NOT fire resolve for a non-aggregator posting with a short description', async () => {
    const posting = makePosting('non-agg-short', {
      source: 'linkedin',
      description: 'Short linkedin snippet.',
    });
    await openPane(posting);

    expect(mockUseResolveJobUrl).toHaveBeenCalledWith(posting.url, false);
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
    expect(screen.getByText('Short linkedin snippet.')).toBeInTheDocument();
  });

  it('does NOT fire resolve for an aggregator posting whose description exceeds the threshold', async () => {
    const posting = makePosting('agg-long', {
      source: 'aggregator',
      description: 'x'.repeat(750),
    });
    await openPane(posting);

    expect(mockUseResolveJobUrl).toHaveBeenCalledWith(posting.url, false);
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
  });
});

describe('JobDetailPane — keep-longer merge', () => {
  it('shows the resolved description when it is longer than the snippet', async () => {
    const snippet = 'Short Adzuna snippet.';
    const fullDesc = 'This is the full job description fetched from the target page.';
    resolveSettled(fullDesc);
    await openPane(makePosting('agg-resolved', { source: 'aggregator', description: snippet }));

    expect(screen.getByText(fullDesc)).toBeInTheDocument();
    expect(screen.queryByText(snippet)).not.toBeInTheDocument();
  });

  it('keeps the original snippet when resolve returns something shorter', async () => {
    const snippet = 'Original Adzuna snippet that is longer than the resolved result.';
    resolveSettled('Tiny.');
    await openPane(makePosting('agg-degraded', { source: 'aggregator', description: snippet }));

    expect(screen.getByText(snippet)).toBeInTheDocument();
    expect(screen.queryByText('Tiny.')).not.toBeInTheDocument();
  });
});

describe('JobDetailPane — load full description button', () => {
  const shortAggregator = (id: string) =>
    makePosting(id, { source: 'aggregator', description: 'Short snippet.' });

  it('shows the button when aggregator posting has a short snippet and resolve has not fetched', async () => {
    await openPane(shortAggregator('agg-btn'));

    expect(screen.getByText('jobs.loadFullDescription')).toBeInTheDocument();
  });

  it('hides the button while resolve is in-flight (isFetching=true)', async () => {
    resolveInFlight();
    await openPane(shortAggregator('agg-btn-fetching'));

    // Snippet is shown immediately; inline updating hint visible; full-load button hidden.
    expect(screen.getByText('Short snippet.')).toBeInTheDocument();
    expect(screen.getByText('jobs.updatingDescription')).toBeInTheDocument();
    expect(screen.queryByText('jobs.loadFullDescription')).not.toBeInTheDocument();
  });

  it('hides the button once resolve returns a longer description', async () => {
    const fullDesc = 'This is the much longer full job description from the redirect target.';
    resolveSettled(fullDesc);
    await openPane(shortAggregator('agg-btn-hidden'));

    expect(screen.queryByText('jobs.loadFullDescription')).not.toBeInTheDocument();
    expect(screen.getByText(fullDesc)).toBeInTheDocument();
  });

  it('does NOT show the button for non-aggregator postings with a full description', async () => {
    await openPane(
      makePosting('non-agg-full', {
        source: 'linkedin',
        description: 'A complete job description.',
      })
    );

    expect(screen.queryByText('jobs.loadFullDescription')).not.toBeInTheDocument();
  });

  it('button is keyboard-reachable (role=button accessible)', async () => {
    await openPane(shortAggregator('agg-btn-a11y'));

    const btn = screen.getByRole('button', { name: /jobs\.loadFullDescription/i });
    expect(btn).toBeInTheDocument();
  });

  it('clicking the button calls resolved.refetch()', async () => {
    // idleStub: isFetched=false, isFetching=false — button is visible.
    // aggregator source + short snippet satisfies the showLoadButton gate.
    await openPane(shortAggregator('btn-refetch'));

    const btn = screen.getByRole('button', { name: /jobs\.loadFullDescription/i });
    expect(btn).toBeInTheDocument();

    await act(async () => {
      btn.click();
    });

    expect(mockRefetch).toHaveBeenCalledTimes(1);
  });
});

describe('JobDetailPane — resolve error state', () => {
  const failedResolve = { isFetched: true, isError: true };

  it('shows error hint when resolve fails for an aggregator short-snippet posting', async () => {
    resolveReturns(failedResolve);
    await openPane(
      makePosting('agg-error', { source: 'aggregator', description: 'Short snippet.' })
    );

    expect(screen.getByText('jobs.descriptionLoadError')).toBeInTheDocument();
  });

  it('retry button remains visible alongside the error hint', async () => {
    resolveReturns(failedResolve);
    await openPane(
      makePosting('agg-error-retry', { source: 'aggregator', description: 'Short snippet.' })
    );

    expect(screen.getByText('jobs.loadFullDescription')).toBeInTheDocument();
  });

  it('does NOT show error hint for a non-aggregator posting', async () => {
    resolveReturns({ isError: true });
    await openPane(
      makePosting('non-agg-no-error-hint', {
        source: 'linkedin',
        description: 'A full linkedin description.',
      })
    );

    expect(screen.queryByText('jobs.descriptionLoadError')).not.toBeInTheDocument();
  });
});
