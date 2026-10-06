import { useRef, useState } from 'react';
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Drawer } from './Drawer';

/**
 * `vanish`: the trigger unmounts while the drawer is open. `fallbackLabel`: adds an
 * always-present control wired as `returnFocusTo`.
 */
function Harness({
  vanish = false,
  fallbackLabel,
  openerLabel = 'open drawer',
}: {
  vanish?: boolean;
  fallbackLabel?: string;
  openerLabel?: string;
}) {
  const [open, setOpen] = useState(false);
  const fallback = useRef<HTMLButtonElement>(null);
  return (
    <>
      {fallbackLabel && <button ref={fallback}>{fallbackLabel}</button>}
      {!(vanish && open) && <button onClick={() => setOpen(true)}>{openerLabel}</button>}
      <Drawer
        open={open}
        onClose={() => setOpen(false)}
        ariaLabel="Filters"
        returnFocusTo={fallbackLabel ? fallback : undefined}
      >
        <button>inside</button>
      </Drawer>
    </>
  );
}

describe('Drawer focus return', () => {
  it('returns focus to the control that opened it when it closes', async () => {
    render(<Harness />);

    const opener = screen.getByRole('button', { name: 'open drawer' });
    await userEvent.click(opener);
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'inside' }));

    await userEvent.keyboard('{Escape}');
    expect(document.activeElement).toBe(opener);
  });

  it('does not throw when the opener is gone by the time it closes', async () => {
    render(<Harness vanish />);

    await userEvent.click(screen.getByRole('button', { name: 'open drawer' }));
    await userEvent.keyboard('{Escape}');

    expect(screen.getByRole('button', { name: 'open drawer' })).toBeInTheDocument();
  });

  it('falls back to returnFocusTo when closing also unmounts the opener', async () => {
    // Models the first-run path: the empty-state CTA opens the drawer, and the
    // drawer's own action (start a scrape) replaces that empty state — so the
    // opener and the drawer disappear in the SAME commit.
    function ClosingHarness() {
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
    render(<ClosingHarness />);

    await userEvent.click(screen.getByRole('button', { name: 'transient opener' }));
    await userEvent.keyboard('{Escape}');

    // Without the fallback focus would land on <body> — a WCAG 2.4.3 failure.
    expect(screen.queryByRole('button', { name: 'transient opener' })).not.toBeInTheDocument();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'always here' }));
  });

  it('never parks focus on <body> when the opener is already gone at open time', async () => {
    // Degenerate case: the trigger unmounts as the drawer opens, so the captured
    // "opener" is whatever activeElement degraded to — `<body>`.
    render(<Harness vanish fallbackLabel="always here" openerLabel="vanishing opener" />);

    await userEvent.click(screen.getByRole('button', { name: 'vanishing opener' }));
    await userEvent.keyboard('{Escape}');

    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'always here' }));
  });

  it('prefers the live opener over returnFocusTo when both exist', async () => {
    render(<Harness fallbackLabel="fallback" openerLabel="opener" />);

    const opener = screen.getByRole('button', { name: 'opener' });
    await userEvent.click(opener);
    await userEvent.keyboard('{Escape}');

    expect(document.activeElement).toBe(opener);
  });
});
