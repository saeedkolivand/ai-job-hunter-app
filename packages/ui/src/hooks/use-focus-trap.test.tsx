import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { useFocusTrap } from './use-focus-trap';

function Trapped({ active }: { active: boolean }) {
  const ref = useFocusTrap(active);
  return (
    <div ref={ref as React.RefObject<HTMLDivElement>}>
      <button>first</button>
      <button>last</button>
    </div>
  );
}

describe('useFocusTrap', () => {
  it('focuses the first focusable element when activated', () => {
    render(<Trapped active />);
    expect(screen.getByRole('button', { name: 'first' })).toHaveFocus();
  });

  it('wraps focus from the last element back to the first on Tab', async () => {
    render(<Trapped active />);
    const last = screen.getByRole('button', { name: 'last' });
    last.focus();
    await userEvent.tab();
    expect(screen.getByRole('button', { name: 'first' })).toHaveFocus();
  });

  it('wraps focus backwards from the first element on Shift+Tab', async () => {
    render(<Trapped active />);
    screen.getByRole('button', { name: 'first' }).focus();
    await userEvent.tab({ shift: true });
    expect(screen.getByRole('button', { name: 'last' })).toHaveFocus();
  });

  it('does nothing when inactive', () => {
    render(
      <>
        <button>outside</button>
        <Trapped active={false} />
      </>
    );
    expect(screen.getByRole('button', { name: 'first' })).not.toHaveFocus();
  });
});
