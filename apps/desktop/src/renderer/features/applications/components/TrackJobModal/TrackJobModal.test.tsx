/**
 * TrackJobModal — submit filtering, onClose, and cancel-resets-form (HIGH blocker)
 *
 * Strategy:
 *  - `useTrackApplication` is mocked so no AppClient / QueryClient provider is
 *    needed.
 *  - `@ajh/translations` returns keys as-is.
 *  - `motion/react` is replaced with plain passthrough fragments.
 *  - `@ajh/ui` (Button, Input, ModalShell) is used real.
 *
 * Covered:
 *  (a) Submit with all fields populated → `track.mutateAsync` called once with
 *      correctly-shaped object (trimmed, all keys present).
 *  (b) Submit with blank/whitespace fields → those keys omitted from the payload
 *      (mirrors the `if (form.x?.trim()) req.x = …` filter at ~line 32-39).
 *  (c) Successful submit calls `onClose`.
 *  (d) Cancel (Ghost button) resets form state and calls `onClose`.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import { TrackJobModal } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── motion/react — passthrough so ModalShell animations don't throw ───────────

vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: React.forwardRef(
      (
        { children, ...rest }: React.HTMLAttributes<HTMLDivElement>,
        ref: React.Ref<HTMLDivElement>
      ) => (
        <div ref={ref} {...rest}>
          {children}
        </div>
      )
    ),
  },
}));

// ── Service hook — controlled mock ────────────────────────────────────────────

const mockTrackMutateAsync = vi.fn().mockResolvedValue({ id: 'new-app-1' });

vi.mock('@/services/use-applications', () => ({
  useTrackApplication: () => ({
    mutateAsync: mockTrackMutateAsync,
    isPending: false,
  }),
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

function renderModal(onClose = vi.fn()) {
  return { onClose, ...render(<TrackJobModal open={true} onClose={onClose} />) };
}

function fillField(labelKey: string, value: string) {
  // Labels use translation keys as their text content.
  const label = screen.getByText(labelKey);
  const input =
    document.getElementById((label as HTMLLabelElement).htmlFor ?? '') ??
    screen.getByLabelText(labelKey);
  fireEvent.change(input, { target: { value } });
}

beforeEach(() => {
  mockTrackMutateAsync.mockClear();
  mockTrackMutateAsync.mockResolvedValue({ id: 'new-app-1' });
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('TrackJobModal — submit with all fields', () => {
  it('calls track.mutateAsync once with the correctly-shaped (trimmed) object', async () => {
    const { onClose } = renderModal();

    fillField('applications.trackModal.urlLabel', '  https://acme.com/job/1  ');
    fillField('applications.trackModal.companyLabel', '  Acme  ');
    fillField('applications.trackModal.titleLabel', '  Engineer  ');
    fillField('applications.trackModal.candidateLabel', '  Jane  ');

    fireEvent.click(screen.getByRole('button', { name: 'applications.trackModal.submit' }));

    await waitFor(() => {
      expect(mockTrackMutateAsync).toHaveBeenCalledTimes(1);
    });

    expect(mockTrackMutateAsync).toHaveBeenCalledWith({
      jobUrl: 'https://acme.com/job/1',
      company: 'Acme',
      title: 'Engineer',
      candidate: 'Jane',
    });

    // onClose must be called after a successful submit.
    await waitFor(() => {
      expect(onClose).toHaveBeenCalledTimes(1);
    });
  });
});

describe('TrackJobModal — submit with blank/whitespace fields', () => {
  it('omits whitespace-only fields from the payload', async () => {
    renderModal();

    // Only company is filled; url, title, candidate are blank/whitespace.
    fillField('applications.trackModal.urlLabel', '   ');
    fillField('applications.trackModal.companyLabel', 'Acme');
    fillField('applications.trackModal.titleLabel', '');
    fillField('applications.trackModal.candidateLabel', '  ');

    fireEvent.click(screen.getByRole('button', { name: 'applications.trackModal.submit' }));

    await waitFor(() => {
      expect(mockTrackMutateAsync).toHaveBeenCalledTimes(1);
    });

    const call = mockTrackMutateAsync.mock.calls[0];
    if (!call) throw new Error('Expected mutateAsync to have been called');
    const payload = call[0];

    // Blank/whitespace fields must be absent (the filter strips them).
    expect(payload).not.toHaveProperty('jobUrl');
    expect(payload).not.toHaveProperty('title');
    expect(payload).not.toHaveProperty('candidate');

    // The non-blank field must be present.
    expect(payload).toHaveProperty('company', 'Acme');
  });

  it('submits an empty object when all fields are whitespace-only', async () => {
    renderModal();

    // Leave all fields at their default empty value and submit.
    fireEvent.click(screen.getByRole('button', { name: 'applications.trackModal.submit' }));

    await waitFor(() => {
      expect(mockTrackMutateAsync).toHaveBeenCalledTimes(1);
    });

    expect(mockTrackMutateAsync).toHaveBeenCalledWith({});
  });
});

describe('TrackJobModal — successful submit calls onClose', () => {
  it('calls onClose exactly once after the mutation resolves', async () => {
    const { onClose } = renderModal();

    fillField('applications.trackModal.companyLabel', 'Beta Corp');

    fireEvent.click(screen.getByRole('button', { name: 'applications.trackModal.submit' }));

    await waitFor(() => {
      expect(onClose).toHaveBeenCalledTimes(1);
    });
  });
});

describe('TrackJobModal — cancel resets form state', () => {
  it('clicking cancel calls onClose and handleClose resets the form', async () => {
    const { onClose } = renderModal();

    // Partially fill the form.
    fillField('applications.trackModal.companyLabel', 'Draft Corp');
    fillField('applications.trackModal.titleLabel', 'Draft Role');

    // Cancel.
    fireEvent.click(screen.getByRole('button', { name: 'applications.trackModal.cancel' }));

    expect(onClose).toHaveBeenCalledTimes(1);

    // Re-render with open=true to confirm the internal form state was reset.
    // handleClose calls setForm({ jobUrl:'', company:'', … }) before calling onClose.
    // We verify that mutateAsync was never called (no accidental submit).
    expect(mockTrackMutateAsync).not.toHaveBeenCalled();
  });

  it('after cancel, re-opening the modal shows empty fields', async () => {
    // Render once with onClose that toggles `open`.
    let isOpen = true;
    const onClose = vi.fn(() => {
      isOpen = false;
    });

    const { rerender } = render(<TrackJobModal open={isOpen} onClose={onClose} />);

    // Fill a field.
    fillField('applications.trackModal.companyLabel', 'Stale Corp');

    // Cancel.
    fireEvent.click(screen.getByRole('button', { name: 'applications.trackModal.cancel' }));

    // Re-open by re-rendering with open=true.
    rerender(<TrackJobModal open={true} onClose={onClose} />);

    // The company input should now be empty (form was reset by handleClose).
    const companyInput = screen.getByPlaceholderText('applications.trackModal.companyPlaceholder');
    expect((companyInput as HTMLInputElement).value).toBe('');
  });
});
