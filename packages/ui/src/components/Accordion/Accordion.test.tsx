import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Accordion } from './Accordion';

describe('Accordion', () => {
  it('is collapsed by default and expands on click', async () => {
    render(<Accordion title="FAQ" content="Hidden answer" />);
    expect(screen.queryByText('Hidden answer')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'FAQ' }));
    expect(screen.getByText('Hidden answer')).toBeInTheDocument();
  });

  it('respects defaultOpen', () => {
    render(<Accordion title="Open" content={<span>node content</span>} defaultOpen />);
    expect(screen.getByText('node content')).toBeInTheDocument();
  });

  it('renders string content as HTML', () => {
    render(<Accordion title="T" content="<b>bold</b>" defaultOpen />);
    expect(screen.getByText('bold')).toBeInTheDocument();
  });
});
