/**
 * PostingListItem — keyboard/click selection, interaction markers, source badge, aria.
 *
 * 2-line compact design with a 32×32 source badge on the left.
 * Viewed rows (opened|viewed interaction, not selected) show a "Viewed" text label
 * and dim the title. Match score is shown only in the detail pane (removed from list rows).
 *
 * Strategy:
 *  - Interaction state comes from posting.interactions.
 *  - onSelect is a vi.fn() spy captured per describe.
 *
 * noUncheckedIndexedAccess: all mock.calls[0] accesses are guarded.
 */

import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── lucide-react — Eye removed (no longer imported by PostingListItem) ─────────

vi.mock('lucide-react', () => ({
  Bookmark: () => <svg aria-hidden="true" data-testid="icon-bookmark" />,
  CircleCheck: () => <svg aria-hidden="true" data-testid="icon-circlecheck" />,
}));

// ── motion/react — forwardRef-safe div stub ───────────────────────────────────

vi.mock('motion/react', () => ({
  motion: {
    div: React.forwardRef(
      (
        { children, ...rest }: React.HTMLAttributes<HTMLDivElement>,
        ref: React.Ref<HTMLDivElement>
      ) => (
        <div ref={ref} {...rest}>
          {children}
        </div>
      )
    ),
  },
}));

// ── @ajh/ui — pass-through stubs ─────────────────────────────────────────────

vi.mock('@ajh/ui', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
  transition: { spring: {} },
}));

// ── component under test ──────────────────────────────────────────────────────

import type { Posting } from '@/features/jobs/types';

import { PostingListItem } from './index';

// ── fixtures ──────────────────────────────────────────────────────────────────

function makePosting(overrides: Partial<Posting> = {}): Posting {
  return {
    id: 'post-1',
    source: 'linkedin',
    externalId: 'ext-1',
    url: 'https://example.com/job/1',
    title: 'Software Engineer',
    company: 'Acme',
    location: 'Berlin',
    description: '',
    capturedAt: 0,
    ...overrides,
  };
}

const formatRelativeTime = () => '2d ago';

// ─────────────────────────────────────────────────────────────────────────────
// handleClick and handleKeyDown — onSelect dispatch
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — click and keyboard selection', () => {
  it('click calls onSelect with the posting', async () => {
    const onSelect = vi.fn();
    const posting = makePosting();
    render(
      <PostingListItem
        posting={posting}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={onSelect}
      />
    );

    await userEvent.click(screen.getByRole('option'));

    expect(onSelect).toHaveBeenCalledTimes(1);
    const arg = onSelect.mock.calls[0]?.[0] as Posting | undefined;
    expect(arg?.id).toBe('post-1');
  });

  it('Enter key calls onSelect with the posting', () => {
    const onSelect = vi.fn();
    const posting = makePosting();
    render(
      <PostingListItem
        posting={posting}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={onSelect}
      />
    );

    fireEvent.keyDown(screen.getByRole('option'), { key: 'Enter' });

    expect(onSelect).toHaveBeenCalledTimes(1);
    const arg = onSelect.mock.calls[0]?.[0] as Posting | undefined;
    expect(arg?.id).toBe('post-1');
  });

  it('Space key calls onSelect with the posting', () => {
    const onSelect = vi.fn();
    const posting = makePosting();
    render(
      <PostingListItem
        posting={posting}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={onSelect}
      />
    );

    fireEvent.keyDown(screen.getByRole('option'), { key: ' ' });

    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it('Tab key does NOT call onSelect', () => {
    const onSelect = vi.fn();
    render(
      <PostingListItem
        posting={makePosting()}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={onSelect}
      />
    );

    fireEvent.keyDown(screen.getByRole('option'), { key: 'Tab' });

    expect(onSelect).not.toHaveBeenCalled();
  });

  it('ArrowDown key does NOT call onSelect', () => {
    const onSelect = vi.fn();
    render(
      <PostingListItem
        posting={makePosting()}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={onSelect}
      />
    );

    fireEvent.keyDown(screen.getByRole('option'), { key: 'ArrowDown' });

    expect(onSelect).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// aria-selected and tabIndex (active-descendant pattern — items never tab stops)
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — aria-selected and tabIndex', () => {
  it('aria-selected is true when selected=true', () => {
    render(
      <PostingListItem
        posting={makePosting()}
        selected={true}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByRole('option')).toHaveAttribute('aria-selected', 'true');
  });

  it('aria-selected is false when selected=false', () => {
    render(
      <PostingListItem
        posting={makePosting()}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByRole('option')).toHaveAttribute('aria-selected', 'false');
  });

  it('tabIndex is -1 when selected (active-descendant: container is the tab stop)', () => {
    render(
      <PostingListItem
        posting={makePosting()}
        selected={true}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByRole('option')).toHaveAttribute('tabindex', '-1');
  });

  it('tabIndex is -1 when not selected', () => {
    render(
      <PostingListItem
        posting={makePosting()}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByRole('option')).toHaveAttribute('tabindex', '-1');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Interaction markers
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — interaction markers', () => {
  it('shows CircleCheck icon when applied interaction present', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [
            {
              interactionType: 'applied',
              jobId: 'post-1',
              timestamp: 0,
              title: 'T',
              company: 'C',
              url: 'u',
              source: 's',
            },
          ],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByTestId('icon-circlecheck')).toBeInTheDocument();
  });

  it('shows "Viewed" text label when opened interaction present (not selected)', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [
            {
              interactionType: 'opened',
              jobId: 'post-1',
              timestamp: 0,
              title: 'T',
              company: 'C',
              url: 'u',
              source: 's',
            },
          ],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    // Assert specifically against the aria-hidden marker span, not the sr-only summary.
    // getAllByText would pass even if only the sr-only node contained the text.
    const ariaHiddenSpans = document.querySelectorAll('[aria-hidden="true"]');
    const viewedLabel = Array.from(ariaHiddenSpans).find((el) => el.textContent === 'jobs.viewed');
    expect(viewedLabel).toBeDefined();
  });

  it('shows "Viewed" text label when viewed interaction present (not selected)', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [
            {
              interactionType: 'viewed',
              jobId: 'post-1',
              timestamp: 0,
              title: 'T',
              company: 'C',
              url: 'u',
              source: 's',
            },
          ],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    const ariaHiddenSpans = document.querySelectorAll('[aria-hidden="true"]');
    const viewedLabel = Array.from(ariaHiddenSpans).find((el) => el.textContent === 'jobs.viewed');
    expect(viewedLabel).toBeDefined();
  });

  it('does NOT show "Viewed" text label when viewed but selected (selected rows never dim)', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [
            {
              interactionType: 'viewed',
              jobId: 'post-1',
              timestamp: 0,
              title: 'T',
              company: 'C',
              url: 'u',
              source: 's',
            },
          ],
        })}
        selected={true}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    // The sr-only summary uses t('jobs.viewed') but the aria-hidden label span must not render.
    // The sr-only span is inside role=option — query specifically for the aria-hidden span.
    const ariaHiddenSpans = document.querySelectorAll('[aria-hidden="true"]');
    const viewedLabel = Array.from(ariaHiddenSpans).find((el) => el.textContent === 'jobs.viewed');
    expect(viewedLabel).toBeUndefined();
  });

  it('shows Bookmark icon when bookmarked interaction present', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [
            {
              interactionType: 'bookmarked',
              jobId: 'post-1',
              timestamp: 0,
              title: 'T',
              company: 'C',
              url: 'u',
              source: 's',
            },
          ],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByTestId('icon-bookmark')).toBeInTheDocument();
  });

  it('shows no icons and no Viewed label when interactions is undefined', () => {
    render(
      <PostingListItem
        posting={makePosting({ interactions: undefined })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.queryByTestId('icon-circlecheck')).not.toBeInTheDocument();
    expect(screen.queryByTestId('icon-bookmark')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.viewed')).not.toBeInTheDocument();
  });

  it('shows only the marker icons whose interaction types are present (no false positives)', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [
            {
              interactionType: 'bookmarked',
              jobId: 'post-1',
              timestamp: 0,
              title: 'T',
              company: 'C',
              url: 'u',
              source: 's',
            },
          ],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByTestId('icon-bookmark')).toBeInTheDocument();
    expect(screen.queryByTestId('icon-circlecheck')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.viewed')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// sr-only status summary — icons/labels are aria-hidden; summary announces to AT
// i18n: the component calls t('jobs.applied') etc., so with the passthrough mock
// the rendered text is the i18n key itself (e.g. "jobs.applied"), not raw English.
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — sr-only status summary', () => {
  it('renders sr-only text using i18n keys when applied+viewed interactions present', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [
            {
              interactionType: 'applied',
              jobId: 'post-1',
              timestamp: 0,
              title: 'T',
              company: 'C',
              url: 'u',
              source: 's',
            },
            {
              interactionType: 'viewed',
              jobId: 'post-1',
              timestamp: 0,
              title: 'T',
              company: 'C',
              url: 'u',
              source: 's',
            },
          ],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    // With the passthrough t mock, t('jobs.applied') → 'jobs.applied'.
    // This confirms the component calls t() rather than hardcoding English strings.
    const srSpan = screen.getByText(/jobs\.applied/);
    expect(srSpan).toBeInTheDocument();
    expect(srSpan.textContent).toContain('jobs.viewed');
  });

  it('renders sr-only text using t("jobs.saved") for the bookmarked state', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [
            {
              interactionType: 'bookmarked',
              jobId: 'post-1',
              timestamp: 0,
              title: 'T',
              company: 'C',
              url: 'u',
              source: 's',
            },
          ],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByText('jobs.saved')).toBeInTheDocument();
  });

  it('does NOT render sr-only summary when no interactions are present', () => {
    render(
      <PostingListItem
        posting={makePosting({ interactions: undefined })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    // No i18n status keys present in the DOM.
    expect(screen.queryByText('jobs.applied')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.viewed')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.saved')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Title dim — text-muted-foreground when viewed && !selected
// cn() is a passthrough so class names are directly assertable on the element.
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — title dim on viewed', () => {
  function viewedPosting(interactionType: 'opened' | 'viewed') {
    return makePosting({
      interactions: [
        {
          interactionType,
          jobId: 'post-1',
          timestamp: 0,
          title: 'T',
          company: 'C',
          url: 'u',
          source: 's',
        },
      ],
    });
  }

  it('title span carries text-muted-foreground when opened interaction present and not selected', () => {
    render(
      <PostingListItem
        posting={viewedPosting('opened')}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    // The title span renders posting.title as its text content.
    const titleSpan = screen.getByText('Software Engineer');
    expect(titleSpan.className).toContain('text-muted-foreground');
  });

  it('title span carries text-muted-foreground when viewed interaction present and not selected', () => {
    render(
      <PostingListItem
        posting={viewedPosting('viewed')}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    const titleSpan = screen.getByText('Software Engineer');
    expect(titleSpan.className).toContain('text-muted-foreground');
  });

  it('title span does NOT carry text-muted-foreground when viewed but selected=true', () => {
    render(
      <PostingListItem
        posting={viewedPosting('viewed')}
        selected={true}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    const titleSpan = screen.getByText('Software Engineer');
    expect(titleSpan.className).not.toContain('text-muted-foreground');
  });

  it('title span does NOT carry text-muted-foreground when no interactions (unviewed)', () => {
    render(
      <PostingListItem
        posting={makePosting({ interactions: undefined })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    const titleSpan = screen.getByText('Software Engineer');
    expect(titleSpan.className).not.toContain('text-muted-foreground');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Source badge — 2-letter abbreviation slot
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — source badge', () => {
  it('renders the first 2 uppercase letters of source as the badge', () => {
    render(
      <PostingListItem
        posting={makePosting({ source: 'linkedin' })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByText('LI')).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Match score intentionally absent from list rows
// (score moved to the detail pane — lock in the removal so it can't silently
//  re-appear in the list without a deliberate test update)
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — no match score in list row', () => {
  it('does not render a match-band element regardless of posting state', () => {
    render(
      <PostingListItem
        posting={makePosting()}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.queryByTestId('match-band')).not.toBeInTheDocument();
  });
});
