import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ModalShell } from './ModalShell';

describe('ModalShell', () => {
  it('renders nothing when closed', () => {
    render(
      <ModalShell open={false} onClose={() => {}}>
        <p>panel body</p>
      </ModalShell>
    );
    expect(screen.queryByText('panel body')).not.toBeInTheDocument();
  });

  it('renders the dialog into a portal when open', () => {
    render(
      <ModalShell open onClose={() => {}}>
        <p>panel body</p>
      </ModalShell>
    );
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText('panel body')).toBeInTheDocument();
  });

  it('closes on Escape', async () => {
    const onClose = vi.fn();
    render(
      <ModalShell open onClose={onClose}>
        <p>body</p>
      </ModalShell>
    );
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalled();
  });

  it('closes when the backdrop is clicked but not when the panel is clicked', async () => {
    const onClose = vi.fn();
    render(
      <ModalShell open onClose={onClose}>
        <button>inside</button>
      </ModalShell>
    );
    await userEvent.click(screen.getByRole('button', { name: 'inside' }));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('does NOT close on backdrop click when closeOnBackdrop={false}', async () => {
    const onClose = vi.fn();
    render(
      <ModalShell open onClose={onClose} closeOnBackdrop={false}>
        <p>body</p>
      </ModalShell>
    );
    // Click the overlay container (the dialog's parent) directly
    const dialog = screen.getByRole('dialog');
    const overlay = dialog.parentElement ?? dialog;
    await userEvent.click(overlay);
    expect(onClose).not.toHaveBeenCalled();
  });

  it('still closes on Escape when closeOnBackdrop={false}', async () => {
    const onClose = vi.fn();
    render(
      <ModalShell open onClose={onClose} closeOnBackdrop={false}>
        <p>body</p>
      </ModalShell>
    );
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalled();
  });

  // --- text-selection drag regression ---

  it('closes when mousedown AND click both land on the backdrop', () => {
    const onClose = vi.fn();
    render(
      <ModalShell open onClose={onClose}>
        <p>body</p>
      </ModalShell>
    );
    const dialog = screen.getByRole('dialog');
    const backdrop = dialog.parentElement!;

    fireEvent.mouseDown(backdrop, { target: backdrop });
    fireEvent.click(backdrop, { target: backdrop });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does NOT close when mousedown starts inside the panel but click reaches the backdrop (selection drag)', () => {
    const onClose = vi.fn();
    render(
      <ModalShell open onClose={onClose}>
        <p>inner content</p>
      </ModalShell>
    );
    const dialog = screen.getByRole('dialog');
    const backdrop = dialog.parentElement!;
    const innerPara = screen.getByText('inner content');

    // Simulate: press starts on a child inside the panel (mousedown on innerPara, which
    // bubbles up to backdrop — so e.target !== e.currentTarget on the backdrop handler).
    fireEvent.mouseDown(innerPara);
    // Release / click lands on the backdrop itself
    fireEvent.click(backdrop, { target: backdrop });

    expect(onClose).not.toHaveBeenCalled();
  });

  it('does NOT close on backdrop mousedown+click when closeOnBackdrop={false}', () => {
    const onClose = vi.fn();
    render(
      <ModalShell open onClose={onClose} closeOnBackdrop={false}>
        <p>body</p>
      </ModalShell>
    );
    const dialog = screen.getByRole('dialog');
    const backdrop = dialog.parentElement!;

    fireEvent.mouseDown(backdrop, { target: backdrop });
    fireEvent.click(backdrop, { target: backdrop });

    expect(onClose).not.toHaveBeenCalled();
  });

  // --- responsive-window-resize contract (header / footer / className) ---

  it('renders nothing when open is false', () => {
    render(
      <ModalShell open={false} onClose={() => {}}>
        <p>hidden</p>
      </ModalShell>
    );
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('renders header → body → footer in DOM order with correct aria attributes', () => {
    render(
      <ModalShell open onClose={() => {}} header={<span>hdr</span>} footer={<span>ftr</span>}>
        <p>body text</p>
      </ModalShell>
    );

    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');

    const hdr = screen.getByText('hdr');
    const body = screen.getByText('body text');
    const ftr = screen.getByText('ftr');

    // All three slots present in the document
    expect(hdr).toBeInTheDocument();
    expect(body).toBeInTheDocument();
    expect(ftr).toBeInTheDocument();

    // DOM order: header before body, body before footer
    expect(hdr.compareDocumentPosition(body) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(body.compareDocumentPosition(ftr) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('body wrapper carries overflow-y-auto + @container; panel carries max-h + flex-col cap classes', () => {
    render(
      <ModalShell open onClose={() => {}} header={<span>h</span>} footer={<span>f</span>}>
        <p>content</p>
      </ModalShell>
    );

    const dialog = screen.getByRole('dialog');

    // Panel (the dialog element itself) must carry the responsive cap classes
    expect(dialog.className).toContain('max-h-[calc(100vh-2rem)]');
    expect(dialog.className).toContain('flex-col');

    // Body wrapper: direct parent of the children content
    const bodyWrapper = screen.getByText('content').parentElement;
    expect(bodyWrapper).not.toBeNull();
    expect(bodyWrapper?.className).toContain('overflow-y-auto');
    expect(bodyWrapper?.className).toContain('@container');
  });

  it('omitting header and footer leaves no empty wrapper elements between panel and body', () => {
    render(
      <ModalShell open onClose={() => {}}>
        <p>only child</p>
      </ModalShell>
    );

    const dialog = screen.getByRole('dialog');
    // Direct children of the panel: only the body wrapper div (no header/footer divs)
    const directChildren = Array.from(dialog.children);
    expect(directChildren).toHaveLength(1);

    // The single child is the scrollable body wrapper
    const [bodyWrapper] = directChildren;
    expect(bodyWrapper?.className).toContain('overflow-y-auto');
  });
});
