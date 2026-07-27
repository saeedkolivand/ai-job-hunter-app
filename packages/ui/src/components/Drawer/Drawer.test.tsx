import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Drawer } from './Drawer';

/** The scrim GlassOverlay renders — the only `aria-hidden` fixed sibling. */
function backdrop(): HTMLElement {
  const el = document.body.querySelector('[aria-hidden="true"]');
  if (!el) throw new Error('backdrop not rendered');
  return el as HTMLElement;
}

describe('Drawer', () => {
  it('renders nothing when closed', () => {
    render(
      <Drawer open={false} onClose={() => {}} ariaLabel="Filters">
        <p>panel body</p>
      </Drawer>
    );
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(screen.queryByText('panel body')).not.toBeInTheDocument();
  });

  it('renders a labelled modal dialog into a portal when open', () => {
    render(
      <Drawer open onClose={() => {}} ariaLabel="Filters">
        <p>panel body</p>
      </Drawer>
    );
    const dialog = screen.getByRole('dialog', { name: 'Filters' });
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog.parentElement).toBe(document.body);
    expect(screen.getByText('panel body')).toBeInTheDocument();
  });

  it('closes on Escape', async () => {
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} ariaLabel="Filters">
        <p>body</p>
      </Drawer>
    );
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does not listen for Escape while closed', async () => {
    const onClose = vi.fn();
    render(
      <Drawer open={false} onClose={onClose} ariaLabel="Filters">
        <p>body</p>
      </Drawer>
    );
    await userEvent.keyboard('{Escape}');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('closes on a backdrop click', async () => {
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} ariaLabel="Filters">
        <p>body</p>
      </Drawer>
    );
    await userEvent.click(backdrop());
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does NOT close on a backdrop click when closeOnBackdrop={false}', async () => {
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} ariaLabel="Filters" closeOnBackdrop={false}>
        <p>body</p>
      </Drawer>
    );
    await userEvent.click(backdrop());
    expect(onClose).not.toHaveBeenCalled();
  });

  it('does not close when a control inside the panel is clicked', async () => {
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} ariaLabel="Filters">
        <button>inside</button>
      </Drawer>
    );
    await userEvent.click(screen.getByRole('button', { name: 'inside' }));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('moves focus into the panel and traps Tab inside it', async () => {
    render(
      <>
        <button>outside</button>
        <Drawer open onClose={() => {}} ariaLabel="Filters">
          <button>first</button>
          <button>last</button>
        </Drawer>
      </>
    );

    const first = screen.getByRole('button', { name: 'first' });
    const last = screen.getByRole('button', { name: 'last' });
    expect(document.activeElement).toBe(first);

    await userEvent.tab();
    expect(document.activeElement).toBe(last);
    // Tab off the last focusable wraps back into the panel — never to `outside`.
    await userEvent.tab();
    expect(document.activeElement).toBe(first);
  });

  it('returns focus to the control that opened it when it closes', async () => {
    function Harness() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button onClick={() => setOpen(true)}>open drawer</button>
          <Drawer open={open} onClose={() => setOpen(false)} ariaLabel="Filters">
            <button>inside</button>
          </Drawer>
        </>
      );
    }
    render(<Harness />);

    const opener = screen.getByRole('button', { name: 'open drawer' });
    await userEvent.click(opener);
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'inside' }));

    await userEvent.keyboard('{Escape}');
    expect(document.activeElement).toBe(opener);
  });

  it('does not throw when the opener is gone by the time it closes', async () => {
    function Harness() {
      const [open, setOpen] = useState(false);
      return (
        <>
          {/* The trigger unmounts while the drawer is open. */}
          {!open && <button onClick={() => setOpen(true)}>open drawer</button>}
          <Drawer open={open} onClose={() => setOpen(false)} ariaLabel="Filters">
            <button>inside</button>
          </Drawer>
        </>
      );
    }
    render(<Harness />);

    await userEvent.click(screen.getByRole('button', { name: 'open drawer' }));
    await userEvent.keyboard('{Escape}');

    expect(screen.getByRole('button', { name: 'open drawer' })).toBeInTheDocument();
  });

  it('pins the panel to the right edge and clamps its width to the window', () => {
    render(
      <Drawer open onClose={() => {}} ariaLabel="Filters">
        <p>body</p>
      </Drawer>
    );
    const dialog = screen.getByRole('dialog');
    expect(dialog.className).toContain('inset-y-0');
    expect(dialog.className).toContain('right-0');
    // Never wider than the window minus a gutter — usable at the 900px floor.
    expect(dialog.className).toContain('w-[30rem]');
    expect(dialog.className).toContain('max-w-[calc(100vw-5rem)]');
  });

  it('accepts aria-labelledby instead of aria-label', () => {
    render(
      <Drawer open onClose={() => {}} ariaLabelledby="drawer-title">
        <h2 id="drawer-title">New scrape</h2>
      </Drawer>
    );
    expect(screen.getByRole('dialog', { name: 'New scrape' })).toBeInTheDocument();
  });
});
