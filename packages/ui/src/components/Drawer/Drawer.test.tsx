import { useRef, useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { variants } from '../../lib/motion';
import { Dropdown } from '../Dropdown';
import { LocationInput } from '../LocationInput';
import { Drawer, drawerTransition, drawerVariants } from './Drawer';

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

  it('frosts the app shell while open and clears it on close (WebView2 workaround)', () => {
    const { rerender } = render(
      <Drawer open={false} onClose={() => {}} ariaLabel="Filters">
        <p>body</p>
      </Drawer>
    );
    expect(document.body.classList.contains('modal-blur-active')).toBe(false);

    rerender(
      <Drawer open onClose={() => {}} ariaLabel="Filters">
        <p>body</p>
      </Drawer>
    );
    // A portaled overlay's own backdrop-filter does not composite reliably under
    // WebView2 — without this body class the drawer's glass is a flat scrim.
    expect(document.body.classList.contains('modal-blur-active')).toBe(true);

    rerender(
      <Drawer open={false} onClose={() => {}} ariaLabel="Filters">
        <p>body</p>
      </Drawer>
    );
    expect(document.body.classList.contains('modal-blur-active')).toBe(false);
  });

  it('falls back to returnFocusTo when closing also unmounts the opener', async () => {
    // Models the first-run path: the empty-state CTA opens the drawer, and the
    // drawer's own action (start a scrape) replaces that empty state — so the
    // opener and the drawer disappear in the SAME commit.
    function Harness() {
      const [phase, setPhase] = useState<'idle' | 'open' | 'done'>('idle');
      const fallback = useRef<HTMLButtonElement>(null);
      return (
        <>
          <button ref={fallback}>always here</button>
          {phase !== 'done' && <button onClick={() => setPhase('open')}>transient opener</button>}
          <Drawer
            open={phase === 'open'}
            onClose={() => setPhase('done')}
            ariaLabel="Filters"
            returnFocusTo={fallback}
          >
            <button>inside</button>
          </Drawer>
        </>
      );
    }
    render(<Harness />);

    await userEvent.click(screen.getByRole('button', { name: 'transient opener' }));
    await userEvent.keyboard('{Escape}');

    // Without the fallback focus would land on <body> — a WCAG 2.4.3 failure.
    expect(screen.queryByRole('button', { name: 'transient opener' })).not.toBeInTheDocument();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'always here' }));
  });

  it('never parks focus on <body> when the opener is already gone at open time', async () => {
    // Degenerate case: the trigger unmounts as the drawer opens, so the captured
    // "opener" is whatever activeElement degraded to — `<body>`.
    function Harness() {
      const [open, setOpen] = useState(false);
      const fallback = useRef<HTMLButtonElement>(null);
      return (
        <>
          <button ref={fallback}>always here</button>
          {!open && <button onClick={() => setOpen(true)}>vanishing opener</button>}
          <Drawer
            open={open}
            onClose={() => setOpen(false)}
            ariaLabel="Filters"
            returnFocusTo={fallback}
          >
            <button>inside</button>
          </Drawer>
        </>
      );
    }
    render(<Harness />);

    await userEvent.click(screen.getByRole('button', { name: 'vanishing opener' }));
    await userEvent.keyboard('{Escape}');

    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'always here' }));
  });

  it('prefers the live opener over returnFocusTo when both exist', async () => {
    function Harness() {
      const [open, setOpen] = useState(false);
      const fallback = useRef<HTMLButtonElement>(null);
      return (
        <>
          <button ref={fallback}>fallback</button>
          <button onClick={() => setOpen(true)}>opener</button>
          <Drawer
            open={open}
            onClose={() => setOpen(false)}
            ariaLabel="Filters"
            returnFocusTo={fallback}
          >
            <button>inside</button>
          </Drawer>
        </>
      );
    }
    render(<Harness />);

    const opener = screen.getByRole('button', { name: 'opener' });
    await userEvent.click(opener);
    await userEvent.keyboard('{Escape}');

    expect(document.activeElement).toBe(opener);
  });

  it('lets an open popover inside consume Escape instead of closing the whole drawer', async () => {
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} ariaLabel="Filters">
        <Dropdown
          options={[
            { value: 'newest', label: 'Newest' },
            { value: 'oldest', label: 'Oldest' },
          ]}
          value="newest"
          onChange={() => {}}
          aria-label="Sort"
        />
      </Drawer>
    );

    const trigger = screen.getByRole('button', { name: 'Sort' });
    await userEvent.click(trigger);
    expect(trigger).toHaveAttribute('aria-expanded', 'true');

    // First Escape: innermost layer only.
    await userEvent.keyboard('{Escape}');
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    expect(onClose).not.toHaveBeenCalled();

    // Second Escape, popover now closed: falls through to the drawer.
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('applies the same Escape layering to a LocationInput inside the drawer', async () => {
    // LocationInput reaches the drawer through ScrapeFilters, so without the
    // guard, dismissing location suggestions tore the whole drawer down with
    // them — the drawer INTRODUCED that regression for a pre-existing input.
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} ariaLabel="Filters">
        <LocationInput id="loc" value="" onChange={() => {}} placeholder="Any location" />
      </Drawer>
    );

    await userEvent.click(screen.getByRole('button', { name: /Any location/ }));

    // The panel focuses its search input on a timer; wait for it so Escape is
    // dispatched at the input that owns the handler.
    const search = await screen.findByPlaceholderText('Search city or postcode…');
    await waitFor(() => expect(search).toHaveFocus());

    await userEvent.keyboard('{Escape}');
    expect(screen.queryByPlaceholderText('Search city or postcode…')).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();

    // Panel closed: Escape now belongs to the drawer.
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

describe('drawerVariants / drawerTransition — reduced-motion seam', () => {
  it('slides horizontally at rest', () => {
    expect(drawerVariants(false)).toBe(variants.slideOverRight);
    expect(drawerVariants(false).initial).toMatchObject({ x: '100%' });
  });

  it('drops the translate ENTIRELY under reduce — opacity only, no positional jump', () => {
    const reduced = drawerVariants(true);
    expect(reduced).toBe(variants.overlay);
    for (const phase of [reduced.initial, reduced.animate, reduced.exit]) {
      expect(phase).not.toHaveProperty('x');
    }
  });

  it('uses a duration proportional to the travel, and zero under reduce', () => {
    // 180ms across a full panel width reads as a snap rather than a slide.
    expect(drawerTransition(false)).toMatchObject({ duration: 0.22 });
    expect(drawerTransition(true)).toMatchObject({ duration: 0 });
  });
});
