import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
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
});
