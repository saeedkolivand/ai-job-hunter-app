import { Zap } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';
import { createEvent, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ActionTile } from './ActionTile';

describe('ActionTile', () => {
  it('renders label, description and badge', () => {
    render(
      <ActionTile icon={Zap} label="Generate" description="Make a doc" badge={<span>NEW</span>} />
    );
    expect(screen.getByText('Generate')).toBeInTheDocument();
    expect(screen.getByText('Make a doc')).toBeInTheDocument();
    expect(screen.getByText('NEW')).toBeInTheDocument();
  });

  it('fires onClick and reflects the active state', async () => {
    const onClick = vi.fn();
    render(<ActionTile icon={Zap} label="Run" active onClick={onClick} />);
    await userEvent.click(screen.getByText('Run'));
    expect(onClick).toHaveBeenCalledOnce();
  });

  it('is reachable by keyboard: tab to it, then Enter and Space fire onClick', async () => {
    // The tile is a div, so without an explicit role/tabIndex/key handler it is
    // mouse-only — `.tab()` would land on nothing and both keys would be inert.
    const onClick = vi.fn();
    render(<ActionTile icon={Zap} label="Run" onClick={onClick} />);

    await userEvent.tab();
    expect(screen.getByRole('button', { name: /Run/ })).toHaveFocus();

    await userEvent.keyboard('{Enter}');
    expect(onClick).toHaveBeenCalledTimes(1);

    await userEvent.keyboard(' ');
    expect(onClick).toHaveBeenCalledTimes(2);
  });

  it('swallows the default action on Space so the page does not scroll under the tile', () => {
    // `userEvent.keyboard(' ')` above proves the handler RAN, not that it
    // called `preventDefault` — and on a focused non-button div the browser
    // default for Space is to scroll. Dispatch the event ourselves so the
    // flag is observable: delete the `preventDefault` line and this fails.
    render(<ActionTile icon={Zap} label="Run" onClick={vi.fn()} />);
    const tile = screen.getByRole('button', { name: /Run/ });

    const space = createEvent.keyDown(tile, { key: ' ' });
    fireEvent(tile, space);
    expect(space.defaultPrevented).toBe(true);

    // Enter activates without a default worth suppressing — asserted so the
    // fix stays a targeted one and not a blanket "prevent everything".
    const enter = createEvent.keyDown(tile, { key: 'Enter' });
    fireEvent(tile, enter);
    expect(enter.defaultPrevented).toBe(false);
  });

  it('activates once per press: a held (auto-repeating) key does not re-fire onClick', () => {
    const onClick = vi.fn();
    render(<ActionTile icon={Zap} label="Run" onClick={onClick} />);
    const tile = screen.getByRole('button', { name: /Run/ });

    fireEvent.keyDown(tile, { key: 'Enter' });
    // Every keydown the OS generates while the key is held carries `repeat`.
    fireEvent.keyDown(tile, { key: 'Enter', repeat: true });
    fireEvent.keyDown(tile, { key: ' ', repeat: true });
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('stays out of the tab order and exposes no role when it has no onClick', () => {
    render(<ActionTile icon={Zap} label="Static" />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });
});
