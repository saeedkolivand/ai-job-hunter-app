import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ResumePreferences } from './index';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

vi.mock('@ajh/ui', async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  useNotification: () => ({ success: vi.fn(), error: vi.fn() }),
}));

const mockRemove = vi.fn().mockResolvedValue(undefined);

vi.mock('@/services', () => ({
  useDocuments: () => ({
    data: [{ _id: 'd1', title: 'My CV', source: 'pdf', createdAt: 1, text: '' }],
    isLoading: false,
  }),
  useRemoveDocument: () => ({ mutateAsync: mockRemove, isPending: false }),
  useSetDefaultDocument: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));
vi.mock('@/hooks/use-import-with-ocr', () => ({
  useImportWithOcr: () => ({ importFile: vi.fn(), isPending: false, isOcr: false }),
}));
vi.mock('@/components/resume/ProfileUrlImport', () => ({ ProfileUrlImport: () => null }));
vi.mock('@/components/contact/ContactConflictModal', () => ({ ContactConflictModal: () => null }));

beforeEach(() => mockRemove.mockClear());

describe('ResumePreferences — delete confirmation', () => {
  it('does not delete on the trash click; deletes only after confirming', async () => {
    const user = userEvent.setup();
    render(<ResumePreferences />);

    await user.click(screen.getByRole('button', { name: 'settings.resume.delete' }));
    expect(mockRemove).not.toHaveBeenCalled();
    expect(screen.getByText('settings.resume.deleteTitle')).toBeInTheDocument();

    const confirm = screen
      .getAllByRole('button', { name: 'settings.resume.delete' })
      .find((b) => b.closest('[role="dialog"], [role="alertdialog"]'));
    expect(confirm).toBeDefined();
    await user.click(confirm as HTMLElement);
    expect(mockRemove).toHaveBeenCalledWith('d1');
  });
});
