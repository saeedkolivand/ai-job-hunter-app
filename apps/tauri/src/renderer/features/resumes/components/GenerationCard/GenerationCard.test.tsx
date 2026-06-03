import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';

import type { AiGenerationRecord } from '@ajh/shared/ipc';

import { GenerationCard } from './index';

// Mock the service barrel (ExternalLink reaches for useOpenExternal) and the
// remove hook so the card renders without a provider tree. We capture the
// mutate spy to prove deletion only fires *after* confirmation.
const mockMutate = vi.fn();

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutate: vi.fn() }),
}));

vi.mock('@/services/use-ai-generations', () => ({
  useRemoveAiGeneration: () => ({ mutate: mockMutate, isPending: false }),
}));

// Translate to the raw key so assertions don't depend on locale resolution.
vi.mock('@/lib/i18n', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const GEN: AiGenerationRecord = {
  id: 'gen-1',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  candidateName: 'Jane Doe',
  createdAt: Date.now(),
  mode: 'ats',
  board: '',
  jobUrl: '',
  resumeText: '',
  coverLetterText: '',
  jobAd: '',
  companyBrief: '',
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  targetLanguage: 'en',
  mismatch: false,
  topRequirements: [],
  applicationAnswers: [],
};

describe('GenerationCard — delete confirmation', () => {
  beforeEach(() => mockMutate.mockClear());

  const DELETE = 'resumes.generated.delete';

  it('does not delete until the confirm dialog is accepted', () => {
    render(<GenerationCard gen={GEN} />);

    // The icon-only trash button is the only delete control before the dialog.
    fireEvent.click(screen.getByRole('button', { name: DELETE }));

    // Clicking the trash must NOT delete immediately — that was the data-loss bug.
    expect(mockMutate).not.toHaveBeenCalled();

    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getByText('resumes.generated.deleteTitle')).toBeInTheDocument();

    // Accept inside the dialog (scoped so we don't match the trash button).
    fireEvent.click(within(dialog).getByRole('button', { name: DELETE }));
    expect(mockMutate).toHaveBeenCalledTimes(1);
    expect(mockMutate).toHaveBeenCalledWith('gen-1');
  });

  it('dismisses without deleting when cancelled', () => {
    render(<GenerationCard gen={GEN} />);

    fireEvent.click(screen.getByRole('button', { name: DELETE }));
    const dialog = screen.getByRole('dialog');
    // ConfirmModal's cancel label is a literal default ("Cancel"), not translated.
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));

    expect(mockMutate).not.toHaveBeenCalled();
  });
});
