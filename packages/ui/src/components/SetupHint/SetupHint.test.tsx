import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { SetupHint } from './SetupHint';

describe('SetupHint', () => {
  it('renders the message and action, firing onAction on click', async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    render(
      <SetupHint message="Connect a provider" actionLabel="Open settings" onAction={onAction} />
    );
    expect(screen.getByText('Connect a provider')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Open settings' }));
    expect(onAction).toHaveBeenCalledTimes(1);
  });

  it('renders nothing when show is false', () => {
    render(<SetupHint show={false} message="hidden message" actionLabel="x" onAction={() => {}} />);
    expect(screen.queryByText('hidden message')).toBeNull();
  });

  it('disables the action and hides its label while pending', () => {
    render(<SetupHint message="m" actionLabel="Connect" onAction={() => {}} pending />);
    expect(screen.getByRole('button')).toBeDisabled();
    expect(screen.queryByText('Connect')).toBeNull();
  });

  it('omits the action button when no actionLabel is given', () => {
    render(<SetupHint message="info only" />);
    expect(screen.getByText('info only')).toBeInTheDocument();
    expect(screen.queryByRole('button')).toBeNull();
  });
});
