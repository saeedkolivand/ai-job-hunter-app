import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { HandEditNudge } from './index';

describe('HandEditNudge', () => {
  it('shows the hint message', () => {
    render(<HandEditNudge />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('dismisses on click and stays hidden across re-renders of the same mount', async () => {
    const user = userEvent.setup();
    const { rerender } = render(<HandEditNudge />);

    await user.click(screen.getByRole('button'));
    expect(screen.queryByRole('status')).toBeNull();

    // An ordinary re-render (no remount) must not resurrect it.
    rerender(<HandEditNudge className="changed" />);
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('reappears on a fresh mount (the "once per generation" contract lives in the host key, not here)', () => {
    const { unmount } = render(<HandEditNudge key="gen-1" />);
    unmount();
    render(<HandEditNudge key="gen-2" />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });
});
