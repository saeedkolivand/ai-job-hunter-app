/**
 * FieldArrayList — presentational contract tests.
 *
 * FieldArrayList takes props (fields, onAppend, onRemove, render, …) directly —
 * it is purely presentational and does NOT call useFieldArray internally.
 * We exercise it by passing in a minimal `fields` array (or empty) and spy
 * functions for onAppend / onRemove.
 *
 * Covers:
 *  - Empty `fields` array → EmptyState is rendered (shows emptyLabel); Add button present.
 *  - N items → N GlassCards are rendered (one per field).
 *  - Clicking the Add button calls `onAppend`.
 *  - Clicking the trash button on a card calls `onRemove` with the correct index.
 *  - The `render` callback is invoked with the correct index for each card.
 */
import { Briefcase } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { FieldArrayList } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function makeFields(count: number) {
  return Array.from({ length: count }, (_, i) => ({ id: `field-${i}` }));
}

interface BaseProps {
  fields?: { id: string }[];
  onAppend?: () => void;
  onRemove?: (i: number) => void;
}

function renderList({ fields = [], onAppend = vi.fn(), onRemove = vi.fn() }: BaseProps = {}) {
  return render(
    <FieldArrayList
      fields={fields}
      onAppend={onAppend}
      onRemove={onRemove}
      addLabel="Add item"
      removeLabel="Remove item"
      emptyLabel="No items yet"
      emptyDescription="Start by adding one."
      icon={Briefcase}
      render={(index) => <span data-testid={`field-content-${index}`}>Item {index}</span>}
    />
  );
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('FieldArrayList — empty state', () => {
  it('renders the emptyLabel when fields is empty', () => {
    renderList({ fields: [] });
    expect(screen.getByText('No items yet')).toBeInTheDocument();
  });

  it('renders the emptyDescription when fields is empty', () => {
    renderList({ fields: [] });
    expect(screen.getByText('Start by adding one.')).toBeInTheDocument();
  });

  it('renders the Add button even in the empty state', () => {
    renderList({ fields: [] });
    expect(screen.getByRole('button', { name: /Add item/i })).toBeInTheDocument();
  });

  it('calls onAppend when Add button is clicked in empty state', async () => {
    const onAppend = vi.fn();
    renderList({ fields: [], onAppend });
    await userEvent.click(screen.getByRole('button', { name: /Add item/i }));
    expect(onAppend).toHaveBeenCalledTimes(1);
  });
});

describe('FieldArrayList — with items', () => {
  it('renders one card per field', () => {
    renderList({ fields: makeFields(3) });
    // Each card's render fn emits a span with data-testid
    expect(screen.getByTestId('field-content-0')).toBeInTheDocument();
    expect(screen.getByTestId('field-content-1')).toBeInTheDocument();
    expect(screen.getByTestId('field-content-2')).toBeInTheDocument();
  });

  it('does NOT show the emptyLabel when there are items', () => {
    renderList({ fields: makeFields(2) });
    expect(screen.queryByText('No items yet')).not.toBeInTheDocument();
  });

  it('renders the Add button below the card list', () => {
    renderList({ fields: makeFields(1) });
    expect(screen.getByRole('button', { name: /Add item/i })).toBeInTheDocument();
  });

  it('calls onAppend when Add button is clicked', async () => {
    const onAppend = vi.fn();
    renderList({ fields: makeFields(1), onAppend });
    await userEvent.click(screen.getByRole('button', { name: /Add item/i }));
    expect(onAppend).toHaveBeenCalledTimes(1);
  });
});

describe('FieldArrayList — remove', () => {
  it('calls onRemove with index 0 when the trash button on the first card is clicked', async () => {
    const onRemove = vi.fn();
    renderList({ fields: makeFields(3), onRemove });
    const trashButtons = screen.getAllByRole('button', { name: /Remove item/i });
    const [firstTrash] = trashButtons;
    expect(firstTrash).toBeDefined();
    await userEvent.click(firstTrash as HTMLElement);
    expect(onRemove).toHaveBeenCalledWith(0);
  });

  it('calls onRemove with the correct index for a middle card', async () => {
    const onRemove = vi.fn();
    renderList({ fields: makeFields(3), onRemove });
    const trashButtons = screen.getAllByRole('button', { name: /Remove item/i });
    const middleTrash = trashButtons[1];
    expect(middleTrash).toBeDefined();
    await userEvent.click(middleTrash as HTMLElement);
    expect(onRemove).toHaveBeenCalledWith(1);
  });

  it('renders exactly one trash button per card', () => {
    renderList({ fields: makeFields(4) });
    const trashButtons = screen.getAllByRole('button', { name: /Remove item/i });
    expect(trashButtons).toHaveLength(4);
  });
});

describe('FieldArrayList — render callback indices', () => {
  it('passes sequential indices (0, 1, 2) to the render callback', () => {
    renderList({ fields: makeFields(3) });
    expect(screen.getByTestId('field-content-0')).toHaveTextContent('Item 0');
    expect(screen.getByTestId('field-content-1')).toHaveTextContent('Item 1');
    expect(screen.getByTestId('field-content-2')).toHaveTextContent('Item 2');
  });
});
