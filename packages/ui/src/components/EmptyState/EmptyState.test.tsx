import { Inbox } from 'lucide-react';
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { EmptyState } from './EmptyState';

describe('EmptyState', () => {
  it('renders the title only', () => {
    render(<EmptyState icon={Inbox} title="Nothing here" />);
    expect(screen.getByText('Nothing here')).toBeInTheDocument();
  });

  it('renders description and action when provided', () => {
    render(
      <EmptyState
        icon={Inbox}
        title="Empty"
        description="No items yet"
        action={<button>Add</button>}
      />
    );
    expect(screen.getByText('No items yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add' })).toBeInTheDocument();
  });
});
