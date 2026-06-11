import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { Alert } from './Alert';

describe('Alert', () => {
  it('renders the message', () => {
    render(<Alert message="Heads up" />);
    expect(screen.getByText('Heads up')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('renders an optional description', () => {
    render(<Alert type="warning" message="Title" description="More detail here" />);
    expect(screen.getByText('Title')).toBeInTheDocument();
    expect(screen.getByText('More detail here')).toBeInTheDocument();
  });

  it('shows no close button by default and a working one when closable', () => {
    const { rerender } = render(<Alert message="x" />);
    expect(screen.queryByLabelText('Close alert')).not.toBeInTheDocument();

    const onClose = vi.fn();
    rerender(<Alert message="x" closable onClose={onClose} />);
    fireEvent.click(screen.getByLabelText('Close alert'));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('x')).not.toBeInTheDocument(); // unmounts itself
  });

  it('renders an action slot', () => {
    render(<Alert message="x" action={<button>Undo</button>} />);
    expect(screen.getByText('Undo')).toBeInTheDocument();
  });

  it('supports each type without throwing', () => {
    for (const type of ['success', 'info', 'warning', 'error'] as const) {
      const { unmount } = render(<Alert type={type} message={`m-${type}`} showIcon />);
      expect(screen.getByText(`m-${type}`)).toBeInTheDocument();
      unmount();
    }
  });
});
