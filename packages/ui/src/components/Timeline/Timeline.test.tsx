import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { Timeline } from './Timeline';

describe('Timeline', () => {
  it('renders each item content and label', () => {
    render(
      <Timeline
        items={[
          { children: 'Created', label: 'Jan 1', color: 'green' },
          { children: 'Applied', label: 'Jan 2', color: 'brand' },
        ]}
      />
    );
    expect(screen.getByText('Created')).toBeInTheDocument();
    expect(screen.getByText('Applied')).toBeInTheDocument();
    expect(screen.getByText('Jan 1')).toBeInTheDocument();
    expect(screen.getByText('Jan 2')).toBeInTheDocument();
  });

  it('renders one list item per node', () => {
    render(<Timeline items={[{ children: 'a' }, { children: 'b' }, { children: 'c' }]} />);
    expect(screen.getAllByRole('listitem')).toHaveLength(3);
  });

  it('appends a pending node at the end', () => {
    render(<Timeline items={[{ children: 'Done' }]} pending="Waiting…" />);
    const items = screen.getAllByRole('listitem');
    expect(items).toHaveLength(2);
    expect(items[1]).toHaveTextContent('Waiting…');
  });

  it('reverse shows items newest-first and moves pending to the top', () => {
    render(
      <Timeline reverse pending="Latest…" items={[{ children: 'first' }, { children: 'second' }]} />
    );
    const items = screen.getAllByRole('listitem');
    expect(items[0]).toHaveTextContent('Latest…');
    expect(items[1]).toHaveTextContent('second');
    expect(items[2]).toHaveTextContent('first');
  });

  it('renders a custom dot node', () => {
    render(<Timeline items={[{ children: 'X', dot: <span data-testid="custom-dot" /> }]} />);
    expect(screen.getByTestId('custom-dot')).toBeInTheDocument();
  });

  it('renders opposite-side labels in alternate mode', () => {
    render(
      <Timeline
        mode="alternate"
        items={[
          { label: 'L1', children: 'C1' },
          { label: 'L2', children: 'C2' },
        ]}
      />
    );
    expect(screen.getByText('L1')).toBeInTheDocument();
    expect(screen.getByText('C2')).toBeInTheDocument();
  });
});
