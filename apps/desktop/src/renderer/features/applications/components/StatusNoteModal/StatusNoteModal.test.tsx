/**
 * StatusNoteModal — the optional-note prompt shared by the row Dropdown, the
 * detail header Dropdown and the Timeline's "Add note".
 *
 * `@ajh/ui` is real (ModalShell's dialog role / focus trap are exercised); only
 * `@ajh/translations` is stubbed so keys assert directly.
 */

import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { StatusNoteModal } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe('StatusNoteModal', () => {
  it('renders nothing while closed', () => {
    render(
      <StatusNoteModal open={false} onClose={vi.fn()} status="applied" onSave={vi.fn()} />
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('is a labelled modal dialog when open', () => {
    render(<StatusNoteModal open onClose={vi.fn()} status="applied" onSave={vi.fn()} />);

    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAccessibleName('applications.note.title');
  });

  it('uses the post-transition copy when `changed`, the plain copy otherwise', () => {
    const { unmount } = render(
      <StatusNoteModal open changed onClose={vi.fn()} status="offer" onSave={vi.fn()} />
    );
    expect(screen.getByText('applications.note.afterChange')).toBeInTheDocument();
    expect(screen.queryByText('applications.note.current')).not.toBeInTheDocument();
    unmount();

    render(<StatusNoteModal open onClose={vi.fn()} status="offer" onSave={vi.fn()} />);
    expect(screen.getByText('applications.note.current')).toBeInTheDocument();
  });

  it('saves the trimmed note and closes', () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<StatusNoteModal open onClose={onClose} status="offer" onSave={onSave} />);

    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: '  spoke to the recruiter  ' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.note.save' }));

    expect(onSave).toHaveBeenCalledWith('spoke to the recruiter');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Skip closes WITHOUT saving', () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<StatusNoteModal open onClose={onClose} status="offer" onSave={onSave} />);

    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: 'discarded' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.note.skip' }));

    expect(onSave).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Escape closes without saving (the prompt is never a blocker)', () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<StatusNoteModal open onClose={onClose} status="offer" onSave={onSave} />);

    fireEvent.keyDown(window, { key: 'Escape' });

    expect(onSave).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('a discarded draft does not survive a re-open', () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <StatusNoteModal open onClose={onClose} status="offer" onSave={vi.fn()} />
    );

    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: 'abandoned draft' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.note.skip' }));

    rerender(<StatusNoteModal open={false} onClose={onClose} status="offer" onSave={vi.fn()} />);
    rerender(<StatusNoteModal open onClose={onClose} status="offer" onSave={vi.fn()} />);

    const field = screen.getByPlaceholderText('applications.note.placeholder');
    expect((field as HTMLTextAreaElement).value).toBe('');
  });
});
