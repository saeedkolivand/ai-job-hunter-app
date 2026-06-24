/**
 * PostingListItem — keyboard/click selection, interaction markers, MatchBand, aria.
 *
 * The 2-line compact design (no avatar, no SourceBadge, icon-only status markers).
 *
 * Strategy:
 *  - Component is fully rendered (MatchBand stubbed with data-testid).
 *  - useRowMatchScore is a vi.fn() so score-present / score-absent branches are both tested.
 *  - Interaction state comes from posting.interactions — icon markers use aria-label.
 *  - onSelect is a vi.fn() spy captured per describe.
 *
 * noUncheckedIndexedAccess: all mock.calls[0] accesses are guarded.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { MatchScore } from '@ajh/shared';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── lucide-react — icons are aria-hidden in the real component; find by testid ──

vi.mock('lucide-react', () => ({
  Bookmark: () => <svg aria-hidden="true" data-testid="icon-bookmark" />,
  CircleCheck: () => <svg aria-hidden="true" data-testid="icon-circlecheck" />,
  Eye: () => <svg aria-hidden="true" data-testid="icon-eye" />,
}));

// ── @ajh/ui — pass-through stubs ─────────────────────────────────────────────

vi.mock('@ajh/ui', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

// ── MatchBand stub — data-testid so presence/absence is assertable ────────────

vi.mock('@/lib/match-band', () => ({
  MatchBand: ({ value, subtle }: { value: number; subtle?: boolean }) => (
    <span data-testid="match-band" data-value={value} data-subtle={subtle ? 'true' : 'false'} />
  ),
}));

// ── useRowMatchScore — vi.fn() for per-test score control ─────────────────────

const mockUseRowMatchScore = vi.fn<
  [],
  { score?: MatchScore; pending: boolean; hasResume: boolean }
>(() => ({ score: undefined, pending: false, hasResume: false }));

vi.mock('@/features/jobs/providers', () => ({
  useRowMatchScore: (...args: unknown[]) => mockUseRowMatchScore(...(args as [])),
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

const BASE_SCORE: MatchScore = {
  resumeId: 'r',
  jobId: 'post-1',
  ats: 70,
  semantic: 80,
  combined: 75,
  gaps: [],
  recommendations: [],
};

const formatRelativeTime = () => '2d ago';

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockUseRowMatchScore.mockReturnValue({ score: undefined, pending: false, hasResume: false });
});

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
// aria-selected and roving tabIndex
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

  it('tabIndex is 0 when selected', () => {
    render(
      <PostingListItem
        posting={makePosting()}
        selected={true}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByRole('option')).toHaveAttribute('tabindex', '0');
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
// Interaction markers — icon-only (no text labels in the 2-line design)
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — interaction markers (icon-only)', () => {
  it('shows CircleCheck icon when applied interaction present', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [{ interactionType: 'applied', jobId: 'post-1', createdAt: 0 }],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByTestId('icon-circlecheck')).toBeInTheDocument();
  });

  it('shows Eye icon when opened interaction present', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [{ interactionType: 'opened', jobId: 'post-1', createdAt: 0 }],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByTestId('icon-eye')).toBeInTheDocument();
  });

  it('shows Eye icon when viewed interaction present', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [{ interactionType: 'viewed', jobId: 'post-1', createdAt: 0 }],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByTestId('icon-eye')).toBeInTheDocument();
  });

  it('shows Bookmark icon when bookmarked interaction present', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [{ interactionType: 'bookmarked', jobId: 'post-1', createdAt: 0 }],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByTestId('icon-bookmark')).toBeInTheDocument();
  });

  it('shows no icons when interactions is undefined', () => {
    render(
      <PostingListItem
        posting={makePosting({ interactions: undefined })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.queryByTestId('icon-circlecheck')).not.toBeInTheDocument();
    expect(screen.queryByTestId('icon-eye')).not.toBeInTheDocument();
    expect(screen.queryByTestId('icon-bookmark')).not.toBeInTheDocument();
  });

  it('shows only the marker icons whose interaction types are present (no false positives)', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [{ interactionType: 'bookmarked', jobId: 'post-1', createdAt: 0 }],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByTestId('icon-bookmark')).toBeInTheDocument();
    expect(screen.queryByTestId('icon-circlecheck')).not.toBeInTheDocument();
    expect(screen.queryByTestId('icon-eye')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// sr-only status summary — icons are aria-hidden; summary announces states to AT
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — sr-only status summary', () => {
  it('renders sr-only text listing active states when interactions are present', () => {
    render(
      <PostingListItem
        posting={makePosting({
          interactions: [
            { interactionType: 'applied', jobId: 'post-1', createdAt: 0 },
            { interactionType: 'viewed', jobId: 'post-1', createdAt: 0 },
          ],
        })}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );
    // The sr-only span is in the DOM but visually hidden.
    expect(screen.getByText(/applied/i)).toBeInTheDocument();
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
    // No status text — neither "applied", "viewed", nor "saved"
    expect(screen.queryByText('applied')).not.toBeInTheDocument();
    expect(screen.queryByText('viewed')).not.toBeInTheDocument();
    expect(screen.queryByText('saved')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// MatchBand renders from useRowMatchScore
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingListItem — MatchBand', () => {
  it('renders MatchBand when useRowMatchScore returns a score', () => {
    mockUseRowMatchScore.mockReturnValue({
      score: BASE_SCORE,
      pending: false,
      hasResume: true,
    });

    render(
      <PostingListItem
        posting={makePosting()}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );

    const band = screen.getByTestId('match-band');
    expect(band).toBeInTheDocument();
    expect(band).toHaveAttribute('data-value', '75');
  });

  it('does not render MatchBand when score is undefined', () => {
    mockUseRowMatchScore.mockReturnValue({ score: undefined, pending: false, hasResume: false });

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

  it('passes subtle=true to MatchBand so list rows use muted-neutral for non-High tiers', () => {
    mockUseRowMatchScore.mockReturnValue({
      score: BASE_SCORE,
      pending: false,
      hasResume: true,
    });

    render(
      <PostingListItem
        posting={makePosting()}
        selected={false}
        formatRelativeTime={formatRelativeTime}
        onSelect={vi.fn()}
      />
    );

    expect(screen.getByTestId('match-band')).toHaveAttribute('data-subtle', 'true');
  });
});
