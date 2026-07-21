import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { type CompanyOption, CompanyTypeahead } from './CompanyTypeahead';

/** Narrow a possibly-undefined indexed access to a definite value (no `!`). */
function must<T>(value: T | null | undefined): T {
  if (value == null) throw new Error('expected a value');
  return value;
}

const SUGGESTIONS: CompanyOption[] = [
  { atsKind: 'greenhouse', slug: 'stripe', displayName: 'Stripe', seenCount: 3, starred: false },
  {
    atsKind: 'lever',
    slug: 'ramp',
    displayName: 'Ramp',
    seenCount: 0,
    starred: true,
    curated: true,
  },
];

function setup(overrides: Partial<ComponentProps<typeof CompanyTypeahead>> = {}) {
  const props = {
    selected: [] as string[],
    onAdd: vi.fn(),
    onRemove: vi.fn(),
    query: '',
    onQueryChange: vi.fn(),
    suggestions: SUGGESTIONS,
    onToggleStar: vi.fn(),
    starLabel: 'Watch',
    removeLabel: 'Remove',
    curatedLabel: 'curated',
    placeholder: 'Type a slug…',
    inputTestId: 'ta-input',
    suggestionTestId: 'ta-row',
    starTestId: 'ta-star',
    chipTestId: 'ta-chip',
    ...overrides,
  } satisfies ComponentProps<typeof CompanyTypeahead>;
  return { props, ...render(<CompanyTypeahead {...props} />) };
}

describe('CompanyTypeahead', () => {
  it('renders a removable chip per selected slug', () => {
    const onRemove = vi.fn();
    setup({ selected: ['stripe', 'ramp'], onRemove });
    const chips = screen.getAllByTestId('ta-chip');
    expect(chips).toHaveLength(2);
    fireEvent.click(must(screen.getAllByRole('button', { name: 'Remove' })[0]));
    expect(onRemove).toHaveBeenCalledWith('stripe');
  });

  it('shows merged suggestions once focused and commits a suggestion on click', async () => {
    const onAdd = vi.fn();
    const onQueryChange = vi.fn();
    setup({ onAdd, onQueryChange });
    await userEvent.click(screen.getByTestId('ta-input'));

    const rows = screen.getAllByTestId('ta-row');
    expect(rows).toHaveLength(2);
    // Both displayName (Stripe) and the curated one (Ramp) are shown as text.
    expect(screen.getByText('Stripe')).toBeInTheDocument();
    expect(screen.getByText('Ramp')).toBeInTheDocument();

    // The primary add-button of the first row commits its slug and clears the query.
    fireEvent.click(within(must(rows[0])).getByText('Stripe'));
    expect(onAdd).toHaveBeenCalledWith('stripe');
    expect(onQueryChange).toHaveBeenCalledWith('');
  });

  it('adds free-text on Enter when no suggestion is active (never a dead end)', async () => {
    const onAdd = vi.fn();
    setup({ query: 'unknownco', suggestions: [], onAdd });
    const input = screen.getByTestId('ta-input');
    await userEvent.click(input);
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onAdd).toHaveBeenCalledWith('unknownco');
  });

  it('ArrowDown then Enter commits the highlighted suggestion, not free-text', async () => {
    const onAdd = vi.fn();
    setup({ query: 'r', onAdd });
    const input = screen.getByTestId('ta-input');
    await userEvent.click(input);
    fireEvent.keyDown(input, { key: 'ArrowDown' }); // index 0 → stripe
    fireEvent.keyDown(input, { key: 'ArrowDown' }); // index 1 → ramp
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onAdd).toHaveBeenCalledWith('ramp');
  });

  it('the star toggle fires onToggleStar with the row option', async () => {
    const onToggleStar = vi.fn();
    setup({ onToggleStar });
    await userEvent.click(screen.getByTestId('ta-input'));
    fireEvent.click(must(screen.getAllByTestId('ta-star')[0]));
    expect(onToggleStar).toHaveBeenCalledWith(SUGGESTIONS[0]);
  });

  it('renders the empty-state node when there are no suggestions', async () => {
    setup({ suggestions: [], emptyState: <div>what is a slug?</div> });
    await userEvent.click(screen.getByTestId('ta-input'));
    expect(screen.getByText('what is a slug?')).toBeInTheDocument();
  });

  it('does NOT use role=option (star buttons must not live inside an option — APG)', async () => {
    setup();
    await userEvent.click(screen.getByTestId('ta-input'));
    // Rows are plain buttons, so a screen reader never meets an option with an
    // interactive child. Star is a real toggle with aria-pressed.
    expect(screen.queryAllByRole('option')).toHaveLength(0);
    expect(screen.getAllByTestId('ta-star')[0]).toHaveAttribute('aria-pressed', 'false');
    expect(screen.getAllByTestId('ta-star')[1]).toHaveAttribute('aria-pressed', 'true');
  });

  it('backspace on an empty query removes the last chip', async () => {
    const onRemove = vi.fn();
    setup({ selected: ['stripe', 'ramp'], query: '', onRemove });
    const input = screen.getByTestId('ta-input');
    await userEvent.click(input);
    fireEvent.keyDown(input, { key: 'Backspace' });
    expect(onRemove).toHaveBeenCalledWith('ramp');
  });
});
