import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ConfirmModal } from './ConfirmModal';

describe('ConfirmModal', () => {
  const base = {
    open: true,
    onClose: vi.fn(),
    onConfirm: vi.fn(),
    title: 'Delete item?',
    description: 'This cannot be undone.',
  };

  it('renders title, description and default button labels', () => {
    render(<ConfirmModal {...base} />);
    expect(screen.getByText('Delete item?')).toBeInTheDocument();
    expect(screen.getByText('This cannot be undone.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Confirm' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
  });

  it('fires onConfirm and onClose', async () => {
    const onConfirm = vi.fn();
    const onClose = vi.fn();
    render(
      <ConfirmModal
        {...base}
        onConfirm={onConfirm}
        onClose={onClose}
        confirmText="Yes"
        cancelText="No"
        variant="danger"
      />
    );
    await userEvent.click(screen.getByRole('button', { name: 'Yes' }));
    expect(onConfirm).toHaveBeenCalledOnce();
    await userEvent.click(screen.getByRole('button', { name: 'No' }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('disables the action buttons while confirming', () => {
    render(<ConfirmModal {...base} isConfirming variant="warning" />);
    expect(screen.getByRole('button', { name: 'Confirm' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled();
  });
});
