/**
 * ExportModal — the shared, void-called export trigger for AIGeneratePage,
 * ResumeBuilderPage, and any future `onExport` host.
 *
 * `handle` is invoked via `onClick={() => void handle(id)}` (fire-and-forget),
 * so a rejection from `onExport` used to be an unhandled promise rejection
 * with nothing shown to the user. These tests pin the fix: a rejected export
 * surfaces a toast, logs (without leaking document content), and leaves the
 * modal open for a retry instead of closing on failure.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

import { ExportModal } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const mockNotify = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return { ...actual, useNotification: () => mockNotify };
});

afterEach(() => vi.clearAllMocks());

describe('ExportModal — a rejected export surfaces a toast, not a silent no-op', () => {
  it('shows the thrown message and keeps the modal open on failure', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const onExport = vi.fn().mockRejectedValue(new Error('Export blocked: too many pages.'));
    const onClose = vi.fn();

    render(<ExportModal open onClose={onClose} meta={null} docType="resume" onExport={onExport} />);

    fireEvent.click(screen.getByRole('button', { name: /PDF/ }));

    await waitFor(() =>
      expect(mockNotify.error).toHaveBeenCalledWith({
        message: 'Export blocked: too many pages.',
      })
    );
    expect(onExport).toHaveBeenCalledWith('pdf');
    // Failure must not close the modal — the user needs to see the toast and retry.
    expect(onClose).not.toHaveBeenCalled();
    // Logged for diagnostics, but never the document content/filename.
    expect(consoleError).toHaveBeenCalledWith(
      '[export] failed',
      expect.objectContaining({ format: 'pdf', docType: 'resume' })
    );

    consoleError.mockRestore();
  });

  it('never persists the rejection message — a header URL must not reach the log file', async () => {
    // `validate/mod.rs`'s `header_url_mismatch` puts the offending URL in its
    // message. `log-bridge.ts` forwards console.error into the rotated log file
    // that ships inside a diagnostics bundle, so the message must stay on
    // screen only. Regression for the CWE-532 review finding on #948.
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const secret = 'https://linkedin.com/in/jane-doe-private';
    const onExport = vi
      .fn()
      .mockRejectedValue(new Error(`Export blocked: Header link ${secret} is not allowed.`));

    render(<ExportModal open onClose={vi.fn()} meta={null} docType="resume" onExport={onExport} />);

    fireEvent.click(screen.getByRole('button', { name: /PDF/ }));

    // The user still sees it — the toast is not persisted anywhere.
    await waitFor(() =>
      expect(mockNotify.error).toHaveBeenCalledWith({
        message: `Export blocked: Header link ${secret} is not allowed.`,
      })
    );
    expect(JSON.stringify(consoleError.mock.calls)).not.toContain(secret);
    expect(consoleError).toHaveBeenCalledWith(
      '[export] failed',
      expect.objectContaining({ format: 'pdf', docType: 'resume', error: 'Error' })
    );

    consoleError.mockRestore();
  });

  it('falls back to a localized generic message when the rejection carries none', async () => {
    const onExport = vi.fn().mockRejectedValue('not an Error instance');
    vi.spyOn(console, 'error').mockImplementation(() => {});

    render(<ExportModal open onClose={vi.fn()} meta={null} docType="resume" onExport={onExport} />);

    fireEvent.click(screen.getByRole('button', { name: /PDF/ }));

    await waitFor(() =>
      expect(mockNotify.error).toHaveBeenCalledWith({ message: 'common.exportFailed' })
    );

    vi.mocked(console.error).mockRestore();
  });

  it('closes the modal on a successful export', async () => {
    const onExport = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    render(<ExportModal open onClose={onClose} meta={null} docType="resume" onExport={onExport} />);

    fireEvent.click(screen.getByRole('button', { name: /PDF/ }));

    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(mockNotify.error).not.toHaveBeenCalled();
  });
});
