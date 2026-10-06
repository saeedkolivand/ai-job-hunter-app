import { useState } from 'react';
import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ImagePreview } from './ImagePreview';
import { Harness, SRC_A } from './ImagePreview.support';

describe('ImagePreview — arrow keys are announced and never leak behind the dialog', () => {
  it('advertises the pan keys on the dialog itself', () => {
    render(<Harness />);
    // Panning has no toolbar button, so the arrows are the only route to the
    // rest of a zoomed image — an undocumented shortcut is not a route.
    expect(screen.getByRole('dialog')).toHaveAttribute(
      'aria-keyshortcuts',
      'ArrowUp ArrowDown ArrowLeft ArrowRight'
    );
  });

  it('takes focus off the field behind it when it opens', async () => {
    function WithField() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <input aria-label="Notes" />
          <button type="button" onClick={() => setOpen(true)}>
            open
          </button>
          <ImagePreview
            items={[SRC_A]}
            index={0}
            open={open}
            onIndexChange={() => {}}
            onOpenChange={setOpen}
          />
        </>
      );
    }
    const user = userEvent.setup();
    render(<WithField />);

    const field = screen.getByRole('textbox', { name: 'Notes' });
    field.focus();
    expect(field).toHaveFocus();

    await user.click(screen.getByRole('button', { name: 'open' }));
    // The arrow handler is bound on `window`, so focus left on the field would
    // move its caret on every pan press. The house focus trap moves focus to
    // the first control INSIDE the dialog (the Close button) — the APG dialog
    // pattern, and what makes the shell's own ring unnecessary.
    expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus();
    expect(screen.getByRole('dialog')).toContainElement(document.activeElement as HTMLElement);
    expect(field).not.toHaveFocus();
  });

  it('contains Tab inside the dialog: forward from the last control wraps to the first', async () => {
    const user = userEvent.setup();
    render(
      <>
        <button type="button">behind the dialog</button>
        <ImagePreview
          items={[SRC_A]}
          index={0}
          open
          onIndexChange={() => {}}
          onOpenChange={() => {}}
        />
      </>
    );

    // `aria-modal` tells a screen reader the page behind is inert; it moves no
    // tab stop, so without a trap Tab walks out into a page the user cannot see.
    const reset = screen.getByRole('button', { name: 'Reset' });
    reset.focus();
    await user.tab();

    expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus();
    expect(screen.getByRole('button', { name: 'behind the dialog' })).not.toHaveFocus();
  });

  it('contains Shift+Tab too: backward from the first control wraps to the last', async () => {
    const user = userEvent.setup();
    render(
      <>
        <button type="button">behind the dialog</button>
        <ImagePreview
          items={[SRC_A]}
          index={0}
          open
          onIndexChange={() => {}}
          onOpenChange={() => {}}
        />
      </>
    );

    // The direction the forward-only guard misses: Shift+Tab from the FIRST
    // control is one keypress from the page behind the lightbox.
    screen.getByRole('button', { name: 'Close' }).focus();
    await user.tab({ shift: true });

    expect(screen.getByRole('button', { name: 'Reset' })).toHaveFocus();
    expect(screen.getByRole('button', { name: 'behind the dialog' })).not.toHaveFocus();
  });

  it('hands focus back to whatever opened it when it closes', async () => {
    function WithTrigger() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            open
          </button>
          <ImagePreview
            items={[SRC_A]}
            index={0}
            open={open}
            onIndexChange={() => {}}
            onOpenChange={setOpen}
          />
        </>
      );
    }
    const user = userEvent.setup();
    render(<WithTrigger />);
    const trigger = screen.getByRole('button', { name: 'open' });

    await user.click(trigger);
    expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus();

    fireEvent.keyDown(window, { key: 'Escape' });

    // Taking focus and never giving it back drops the user on `<body>`: the
    // next Tab restarts at the top of the page instead of at the thumbnail
    // they opened the preview from.
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });
});
