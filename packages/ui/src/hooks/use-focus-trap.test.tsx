import { describe, expect, it } from 'vitest';
import { act, render, screen } from '@testing-library/react';
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

/** Simulates a panel whose content churns while open (loading → rows →
 *  extra footer action) WITHOUT `active` ever toggling — the exact case that
 *  broke a one-time focusable snapshot. */
function ChurningTrapped({ expanded }: { expanded: boolean }) {
  const ref = useFocusTrap(true);
  return (
    <div ref={ref as React.RefObject<HTMLDivElement>}>
      <button>first</button>
      {expanded && <button>new-last</button>}
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

  it('re-queries focusables on every Tab, so a newly-added last element traps correctly (no stale snapshot)', async () => {
    const { rerender } = render(<ChurningTrapped expanded={false} />);

    // Content churns while the trap stays active the whole time (`active`
    // never toggles) — a snapshot taken once at mount would still think
    // "first" is the only/last element.
    await act(async () => {
      rerender(<ChurningTrapped expanded />);
    });

    screen.getByRole('button', { name: 'new-last' }).focus();
    await userEvent.tab();
    // Tab from the NEW last element must wrap to "first", not escape the trap.
    expect(screen.getByRole('button', { name: 'first' })).toHaveFocus();
  });

  // Closing dumped focus on <body>: a keyboard user restarted from the top of
  // the document instead of the control that opened the dialog.
  describe('return focus', () => {
    it('hands focus back to the opener when the trap deactivates', () => {
      const { rerender } = render(
        <>
          <button>opener</button>
          <Trapped active={false} />
        </>
      );

      const opener = screen.getByRole('button', { name: 'opener' });
      opener.focus();
      expect(opener).toHaveFocus();

      rerender(
        <>
          <button>opener</button>
          <Trapped active />
        </>
      );
      expect(screen.getByRole('button', { name: 'first' })).toHaveFocus();

      rerender(
        <>
          <button>opener</button>
          <Trapped active={false} />
        </>
      );
      expect(opener).toHaveFocus();
    });

    it('restores on unmount too (the dialog is removed, not just deactivated)', () => {
      const { rerender } = render(
        <>
          <button>opener</button>
        </>
      );
      const opener = screen.getByRole('button', { name: 'opener' });
      opener.focus();

      rerender(
        <>
          <button>opener</button>
          <Trapped active />
        </>
      );
      expect(screen.getByRole('button', { name: 'first' })).toHaveFocus();

      rerender(
        <>
          <button>opener</button>
        </>
      );
      expect(opener).toHaveFocus();
    });

    // The restore is a FALLBACK: a wrapper with its own return-focus target
    // (e.g. a Drawer's `returnFocusTo`) must still win.
    it('does NOT steal focus that something else already moved somewhere meaningful', () => {
      const { rerender } = render(
        <>
          <button>opener</button>
          <button>elsewhere</button>
          <Trapped active={false} />
        </>
      );

      screen.getByRole('button', { name: 'opener' }).focus();
      rerender(
        <>
          <button>opener</button>
          <button>elsewhere</button>
          <Trapped active />
        </>
      );

      // Someone else claims focus before the trap tears down.
      const elsewhere = screen.getByRole('button', { name: 'elsewhere' });
      elsewhere.focus();

      rerender(
        <>
          <button>opener</button>
          <button>elsewhere</button>
          <Trapped active={false} />
        </>
      );
      expect(elsewhere).toHaveFocus();
    });

    it('does nothing when the remembered element has left the DOM', () => {
      const { rerender } = render(
        <>
          <button>opener</button>
          <Trapped active={false} />
        </>
      );
      screen.getByRole('button', { name: 'opener' }).focus();

      rerender(
        <>
          <button>opener</button>
          <Trapped active />
        </>
      );

      // The opener unmounts while the trap is open — restoring to it would throw
      // or focus a detached node.
      expect(() =>
        rerender(
          <>
            <Trapped active={false} />
          </>
        )
      ).not.toThrow();
    });
  });
});
