import { type ComponentProps, createRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import {
  type CompanyOption,
  CompanyTypeahead,
  type CompanyTypeaheadHandle,
} from './CompanyTypeahead';

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
    // Per-company labels — a screen reader hears "Watch Stripe" / "Remove stripe"
    // rather than N identical controls.
    starLabel: (o: CompanyOption) => `Watch ${o.displayName || o.slug}`,
    removeLabel: (slug: string) => `Remove ${slug}`,
    resultsLabel: (n: number) => `${n} found`,
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
  it('renders a removable chip per selected slug with a per-company label', () => {
    const onRemove = vi.fn();
    setup({ selected: ['stripe', 'ramp'], onRemove });
    expect(screen.getAllByTestId('ta-chip')).toHaveLength(2);
    // Name-based, per-company: no positional [0] guess.
    fireEvent.click(screen.getByRole('button', { name: 'Remove stripe' }));
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

  it('commitPending() via ref commits the pending query synchronously and is idempotent', () => {
    const ref = createRef<CompanyTypeaheadHandle>();
    const onAdd = vi.fn();
    const onQueryChange = vi.fn();
    const { props, rerender } = setup({
      query: 'acme',
      suggestions: [],
      onAdd,
      onQueryChange,
      ref,
    });

    act(() => ref.current?.commitPending());
    // Synchronous: onAdd ran inside commitPending, not on a later tick.
    expect(onAdd).toHaveBeenCalledWith('acme');
    expect(onAdd).toHaveBeenCalledTimes(1);
    expect(onQueryChange).toHaveBeenCalledWith('');

    // Once the parent has cleared the query, a repeat flush is a no-op — so it's
    // safe to run alongside the blur-commit (no double-add).
    rerender(<CompanyTypeahead {...props} query="" ref={ref} />);
    act(() => ref.current?.commitPending());
    expect(onAdd).toHaveBeenCalledTimes(1);
  });

  it('commits a typed-but-uncommitted slug on blur (no silent data loss on Start)', async () => {
    const onAdd = vi.fn();
    const onQueryChange = vi.fn();
    setup({ query: 'acme', suggestions: [], onAdd, onQueryChange });
    const input = screen.getByTestId('ta-input');
    await userEvent.click(input);
    // Focus leaves the widget entirely (e.g. onto Start Scrape).
    fireEvent.focusOut(input, { relatedTarget: document.body });
    expect(onAdd).toHaveBeenCalledWith('acme');
    expect(onQueryChange).toHaveBeenCalledWith('');
  });

  it('does NOT commit the query when a suggestion is clicked (no double-add)', async () => {
    const onAdd = vi.fn();
    // query is 'str' (a filter fragment) — clicking a suggestion must add ONLY
    // that suggestion, never also the typed fragment via a stray blur-commit.
    setup({ query: 'str', onAdd });
    await userEvent.click(screen.getByTestId('ta-input'));
    const rows = screen.getAllByTestId('ta-row');
    await userEvent.click(within(must(rows[0])).getByText('Stripe'));
    expect(onAdd).toHaveBeenCalledTimes(1);
    expect(onAdd).toHaveBeenCalledWith('stripe');
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
    fireEvent.click(screen.getByRole('button', { name: 'Watch Stripe' }));
    expect(onToggleStar).toHaveBeenCalledWith(SUGGESTIONS[0]);
  });

  it('renders the empty-state node when there are no suggestions', async () => {
    setup({ suggestions: [], emptyState: <div>what is a slug?</div> });
    await userEvent.click(screen.getByTestId('ta-input'));
    expect(screen.getByText('what is a slug?')).toBeInTheDocument();
  });

  it('announces the result count in an aria-live status region while searching', async () => {
    setup({ query: 'st', suggestions: [must(SUGGESTIONS[0])] });
    await userEvent.click(screen.getByTestId('ta-input'));
    expect(screen.getByRole('status')).toHaveTextContent('1 found');
  });

  it('does NOT use role=option (star buttons must not live inside an option — APG)', async () => {
    setup();
    await userEvent.click(screen.getByTestId('ta-input'));
    // Rows are plain buttons, so a screen reader never meets an option with an
    // interactive child. State is a real toggle button with aria-pressed.
    expect(screen.queryAllByRole('option')).toHaveLength(0);
    expect(screen.getByRole('button', { name: 'Watch Stripe' })).toHaveAttribute(
      'aria-pressed',
      'false'
    );
    expect(screen.getByRole('button', { name: 'Watch Ramp' })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
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
