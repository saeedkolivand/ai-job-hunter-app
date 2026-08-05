/**
 * ProfileUrlImport — profile-fetch vs. save-failure handling.
 *
 * `documents_import` resolves (never rejects) on a save failure — an in-band
 * `{ error }` payload, not a thrown mutation error — so a naive
 * `await mutateAsync()` followed by unconditional success effects fires the
 * success toast AND `onImported` on a failed save. These tests pin the fix:
 * the save result is checked for `error` before either happens.
 *
 * They also cover the two backend profile-fetch error strings a real user
 * will actually hit (rate-limited, no public data) — shown verbatim now that
 * the dead "connect your LinkedIn account" auth-wall classifier is gone (the
 * backend fetch is always anonymous; connecting an account is what used to
 * break this).
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as AjhUi from '@ajh/ui';

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

const profileImportMutateAsync = vi.fn();
const importDocumentMutateAsync = vi.fn();

vi.mock('@/services', () => ({
  useProfileImport: () => ({ mutateAsync: profileImportMutateAsync, isPending: false }),
  useImportDocument: () => ({ mutateAsync: importDocumentMutateAsync, isPending: false }),
}));

afterEach(() => vi.clearAllMocks());

import { ProfileUrlImport } from './index';

const LINKEDIN_URL = 'https://linkedin.com/in/someone';

/** Opens the URL field, types a valid LinkedIn URL, and submits. */
async function submitUrl(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByText('resumeInput.importFromLinkedin'));
  await user.type(screen.getByPlaceholderText('resumeInput.profileUrlPlaceholder'), LINKEDIN_URL);
  await user.click(screen.getByText('resumeInput.profileUrlImport'));
}

describe('ProfileUrlImport — profile-fetch errors are shown verbatim', () => {
  it('shows the rate-limited backend message and never attempts a save', async () => {
    const message = 'linkedin rate-limited this request — wait a few minutes and try again';
    profileImportMutateAsync.mockResolvedValue({ error: message });
    const onImported = vi.fn();
    const user = userEvent.setup();
    render(<ProfileUrlImport onImported={onImported} />);

    await submitUrl(user);

    expect(mockNotify.error).toHaveBeenCalledWith({ message });
    expect(importDocumentMutateAsync).not.toHaveBeenCalled();
    expect(onImported).not.toHaveBeenCalled();
  });

  it('shows the no-public-data backend message and never attempts a save', async () => {
    const message =
      'linkedin did not return public profile data for this url — it may be private, unavailable, or the request was blocked';
    profileImportMutateAsync.mockResolvedValue({ error: message });
    const onImported = vi.fn();
    const user = userEvent.setup();
    render(<ProfileUrlImport onImported={onImported} />);

    await submitUrl(user);

    expect(mockNotify.error).toHaveBeenCalledWith({ message });
    expect(onImported).not.toHaveBeenCalled();
  });
});

describe('ProfileUrlImport — a failed save does not fire the success toast', () => {
  it('surfaces the save error and skips onImported when the save resolves { error }', async () => {
    profileImportMutateAsync.mockResolvedValue({ text: 'resume text', name: 'Jane Doe' });
    importDocumentMutateAsync.mockResolvedValue({ error: 'text extraction failed: boom' });
    const onImported = vi.fn();
    const user = userEvent.setup();
    render(<ProfileUrlImport onImported={onImported} />);

    await submitUrl(user);

    expect(mockNotify.error).toHaveBeenCalledWith({ message: 'text extraction failed: boom' });
    expect(mockNotify.success).not.toHaveBeenCalled();
    expect(onImported).not.toHaveBeenCalled();
  });

  it('surfaces the human message for a scanned_pdf save failure', async () => {
    profileImportMutateAsync.mockResolvedValue({ text: 'resume text', name: 'Jane Doe' });
    importDocumentMutateAsync.mockResolvedValue({
      error: 'scanned_pdf',
      message: 'PDF appears to be scanned. Please upload a text-based PDF or DOCX.',
    });
    const onImported = vi.fn();
    const user = userEvent.setup();
    render(<ProfileUrlImport onImported={onImported} />);

    await submitUrl(user);

    expect(mockNotify.error).toHaveBeenCalledWith({
      message: 'PDF appears to be scanned. Please upload a text-based PDF or DOCX.',
    });
    expect(mockNotify.success).not.toHaveBeenCalled();
    expect(onImported).not.toHaveBeenCalled();
  });
});

describe('ProfileUrlImport — successful import end-to-end', () => {
  it('fires the success toast and onImported only after a successful save', async () => {
    profileImportMutateAsync.mockResolvedValue({ text: 'resume text', name: 'Jane Doe' });
    importDocumentMutateAsync.mockResolvedValue({ id: 'doc-1' });
    const onImported = vi.fn();
    const user = userEvent.setup();
    render(<ProfileUrlImport onImported={onImported} />);

    await submitUrl(user);

    expect(mockNotify.success).toHaveBeenCalledWith({ message: 'resumeInput.profileImported' });
    expect(onImported).toHaveBeenCalledWith({ id: 'doc-1', name: 'Jane Doe' });
    expect(mockNotify.error).not.toHaveBeenCalled();
  });
});
