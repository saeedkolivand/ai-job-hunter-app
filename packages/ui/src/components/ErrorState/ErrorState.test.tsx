import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ErrorState } from './ErrorState';

describe('ErrorState', () => {
  it('shows the default title', () => {
    render(<ErrorState />);
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
  });

  it('renders a retry button that calls onRetry', async () => {
    const onRetry = vi.fn();
    render(<ErrorState title="Failed" description="try later" onRetry={onRetry} />);
    expect(screen.getByText('Failed')).toBeInTheDocument();
    expect(screen.getByText('try later')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /try again/i }));
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it('renders a custom action node', () => {
    render(<ErrorState action={<span>custom</span>} />);
    expect(screen.getByText('custom')).toBeInTheDocument();
  });
});
