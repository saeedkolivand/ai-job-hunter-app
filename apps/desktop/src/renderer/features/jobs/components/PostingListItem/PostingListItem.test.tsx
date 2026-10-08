/**
 * PostingListItem — keyboard/click selection, interaction markers, source badge, aria.
 *
 * 2-line compact design with a 32×32 source badge on the left.
 * Viewed rows (opened|viewed interaction, not selected) show a "Viewed" text label
 * and dim the title. Match score is shown only in the detail pane (removed from list rows).
 *
 * Strategy:
 *  - Interaction state comes from posting.interactions.
 *  - onSelect is a vi.fn() spy captured per test.
 *
 * noUncheckedIndexedAccess: all mock.calls[0] accesses are guarded.
 */

import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// Eye removed (no longer imported by PostingListItem).
vi.mock('lucide-react', () => ({
  Bookmark: () => <svg aria-hidden="true" data-testid="icon-bookmark" />,
  CircleCheck: () => <svg aria-hidden="true" data-testid="icon-circlecheck" />,
}));

// forwardRef-safe div stub.
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

vi.mock('@ajh/ui', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
  transition: { spring: {} },
  resolveTransition: (t: unknown) => t,
  // Real chips render through these — SourceBadge/Tag are non-focusable spans so
  // the "no focusable descendants" assertion reflects production semantics.
  SourceBadge: ({ source }: { source: string }) => <span>{source}</span>,
  Tag: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  Button: ({ children, onClick }: { children?: React.ReactNode; onClick?: () => void }) => (
    <button type="button" onClick={onClick}>
      {children}
    </button>
  ),
}));

// Real cluster/agency chips render here (the broken non-interactive path is
// what we assert), so ClusterSourceChips' useOpenExternal needs a stub — the
// only provider dependency (no AppClient/QueryClient tree required).
vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutate: vi.fn() }),
}));

// Renders company initials via passthrough.
vi.mock('@/features/jobs/components/CompanyAvatar', () => ({
  CompanyAvatar: ({ company, sourceFallback }: { company: string; sourceFallback?: string }) => {
    const label = company.trim() || (sourceFallback ?? '');
    const mono = label ? label.slice(0, 2).toUpperCase() : '?';
    return <div aria-hidden="true">{mono}</div>;
  },
}));

import type { Posting } from '@/features/jobs/types';

import { PostingListItem } from './index';

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

type InteractionType = NonNullable<Posting['interactions']>[number]['interactionType'];

/** A posting carrying one interaction of each given type. */
function withInteractions(...types: InteractionType[]): Posting {
  return makePosting({
    interactions: types.map((interactionType) => ({
      interactionType,
      jobId: 'post-1',
      timestamp: 0,
      title: 'T',
      company: 'C',
      url: 'u',
      source: 's',
    })),
  });
}

function renderItem(
  posting: Posting = makePosting(),
  { selected = false, onSelect = vi.fn() }: { selected?: boolean; onSelect?: () => void } = {}
) {
  return render(
    <PostingListItem
      posting={posting}
      selected={selected}
      formatRelativeTime={() => '2d ago'}
      onSelect={onSelect}
    />
  );
}

/** The aria-hidden "Viewed" marker span (NOT the sr-only summary), if rendered. */
function viewedLabel() {
  return Array.from(document.querySelectorAll('[aria-hidden="true"]')).find(
    (el) => el.textContent === 'jobs.viewed'
  );
}

describe('PostingListItem — click and keyboard selection', () => {
  it('click calls onSelect with the posting', async () => {
    const onSelect = vi.fn();
    renderItem(makePosting(), { onSelect });

    await userEvent.click(screen.getByRole('option'));

    expect(onSelect).toHaveBeenCalledTimes(1);
    const arg = onSelect.mock.calls[0]?.[0] as Posting | undefined;
    expect(arg?.id).toBe('post-1');
  });

  // Enter/Space belong to the listbox container (active descendant), not the row.
  it.each(['Tab', 'ArrowDown', 'Enter', ' '])('%s key does NOT call onSelect', (key) => {
    const onSelect = vi.fn();
    renderItem(makePosting(), { onSelect });

    fireEvent.keyDown(screen.getByRole('option'), { key });

    expect(onSelect).not.toHaveBeenCalled();
  });
});

// Active-descendant pattern — items never tab stops.
describe('PostingListItem — aria-selected and tabIndex', () => {
  it.each([true, false])('aria-selected is %s when selected', (selected) => {
    renderItem(makePosting(), { selected });
    expect(screen.getByRole('option')).toHaveAttribute('aria-selected', String(selected));
  });

  it.each([true, false])(
    'tabIndex is -1 when selected is %s (container is the tab stop)',
    (selected) => {
      renderItem(makePosting(), { selected });
      expect(screen.getByRole('option')).toHaveAttribute('tabindex', '-1');
    }
  );
});

describe('PostingListItem — interaction markers', () => {
  it('shows CircleCheck icon when applied interaction present', () => {
    renderItem(withInteractions('applied'));
    expect(screen.getByTestId('icon-circlecheck')).toBeInTheDocument();
  });

  // Assert specifically against the aria-hidden marker span, not the sr-only
  // summary — a text query would pass even if only the sr-only node had the text.
  it.each(['opened', 'viewed'] as const)(
    'shows "Viewed" text label when %s interaction present (not selected)',
    (type) => {
      renderItem(withInteractions(type));
      expect(viewedLabel()).toBeDefined();
    }
  );

  it('does NOT show "Viewed" text label when viewed but selected (selected rows never dim)', () => {
    renderItem(withInteractions('viewed'), { selected: true });
    // The sr-only summary uses t('jobs.viewed') but the aria-hidden label span must not render.
    expect(viewedLabel()).toBeUndefined();
  });

  it('shows Bookmark icon when bookmarked interaction present', () => {
    renderItem(withInteractions('bookmarked'));
    expect(screen.getByTestId('icon-bookmark')).toBeInTheDocument();
  });

  it('shows no icons and no Viewed label when interactions is undefined', () => {
    renderItem(makePosting({ interactions: undefined }));
    expect(screen.queryByTestId('icon-circlecheck')).not.toBeInTheDocument();
    expect(screen.queryByTestId('icon-bookmark')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.viewed')).not.toBeInTheDocument();
  });

  it('shows only the marker icons whose interaction types are present (no false positives)', () => {
    renderItem(withInteractions('bookmarked'));
    expect(screen.getByTestId('icon-bookmark')).toBeInTheDocument();
    expect(screen.queryByTestId('icon-circlecheck')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.viewed')).not.toBeInTheDocument();
  });
});

// Icons/labels are aria-hidden; the summary announces to AT. The component calls
// t('jobs.applied') etc., so with the passthrough mock the rendered text is the
// i18n key itself, not raw English.
describe('PostingListItem — sr-only status summary', () => {
  it('renders sr-only text using i18n keys when applied+viewed interactions present', () => {
    renderItem(withInteractions('applied', 'viewed'));
    // With the passthrough t mock, t('jobs.applied') → 'jobs.applied'.
    // This confirms the component calls t() rather than hardcoding English strings.
    const srSpan = screen.getByText(/jobs\.applied/);
    expect(srSpan).toBeInTheDocument();
    expect(srSpan.textContent).toContain('jobs.viewed');
  });

  it('renders sr-only text using t("jobs.saved") for the bookmarked state', () => {
    renderItem(withInteractions('bookmarked'));
    expect(screen.getByText('jobs.saved')).toBeInTheDocument();
  });

  it('does NOT render sr-only summary when no interactions are present', () => {
    renderItem(makePosting({ interactions: undefined }));
    // No i18n status keys present in the DOM.
    expect(screen.queryByText('jobs.applied')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.viewed')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.saved')).not.toBeInTheDocument();
  });
});

// text-muted-foreground when viewed && !selected. cn() is a passthrough so class
// names are directly assertable on the element.
describe('PostingListItem — title dim on viewed', () => {
  it.each(['opened', 'viewed'] as const)(
    'title span carries text-muted-foreground when %s interaction present and not selected',
    (type) => {
      renderItem(withInteractions(type));
      // The title span renders posting.title as its text content.
      expect(screen.getByText('Software Engineer').className).toContain('text-muted-foreground');
    }
  );

  it('title span does NOT carry text-muted-foreground when viewed but selected=true', () => {
    renderItem(withInteractions('viewed'), { selected: true });
    expect(screen.getByText('Software Engineer').className).not.toContain('text-muted-foreground');
  });

  it('title span does NOT carry text-muted-foreground when no interactions (unviewed)', () => {
    renderItem(makePosting({ interactions: undefined }));
    expect(screen.getByText('Software Engineer').className).not.toContain('text-muted-foreground');
  });
});

describe('PostingListItem — company avatar', () => {
  it('renders company initials via CompanyAvatar', () => {
    renderItem(makePosting({ company: 'Acme' }));
    // CompanyAvatar mock renders first 2 chars of company name uppercased
    expect(screen.getByText('AC')).toBeInTheDocument();
  });

  it('falls back to source initials when company is empty', () => {
    renderItem(makePosting({ company: '', source: 'linkedin' }));
    expect(screen.getByText('LI')).toBeInTheDocument();
  });
});

// Score moved to the detail pane — lock in the removal so it can't silently
// re-appear in the list without a deliberate test update.
describe('PostingListItem — no match score in list row', () => {
  it('does not render a match-band element regardless of posting state', () => {
    renderItem();
    expect(screen.queryByTestId('match-band')).not.toBeInTheDocument();
  });
});

// ADR-029 + APG active-descendant: role="option" with tabIndex=-1 must not
// contain focusable descendants — chips render as presentational badges only.
describe('PostingListItem — cluster/agency chips are non-interactive', () => {
  function clustered(overrides: Partial<Posting> = {}): Posting {
    return makePosting({
      clusterId: 'k1',
      clusterCanonical: true,
      clusterMembers: [
        { key: 'k1', board: 'linkedin', url: 'https://example.com/job/1' },
        { key: 'k2', board: 'indeed', url: 'https://indeed.com/job/2' },
      ],
      ...overrides,
    });
  }

  it('renders source + agency chips with ZERO focusable descendants in the option row', () => {
    renderItem(clustered({ isAgency: true }));
    const row = screen.getByRole('option');
    // Active-descendant pattern: the option must add no tab stops.
    expect(within(row).queryAllByRole('button')).toHaveLength(0);
    expect(within(row).queryAllByRole('link')).toHaveLength(0);
    // The non-self source still renders — as a presentational badge, not a control.
    const chips = within(row).getAllByTestId(TEST_IDS.jobs.clusterSourceChip);
    expect(chips).toHaveLength(1);
    expect(within(row).getByText('indeed')).toBeInTheDocument();
  });

  it('a chip click selects the row once — no double action, no separate handler', () => {
    const onSelect = vi.fn();
    renderItem(clustered(), { onSelect });
    const chip = within(screen.getByRole('option')).getByTestId(TEST_IDS.jobs.clusterSourceChip);
    fireEvent.click(chip);
    // The chip has no own click handler; the click bubbles to the row and
    // selects exactly once (no chip-level second action).
    expect(onSelect).toHaveBeenCalledTimes(1);
  });
});
