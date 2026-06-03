import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, within } from '@testing-library/react';

import type { AiGenerationRecord } from '@ajh/shared/ipc';

import type * as Generate from '@/lib/generate';
import { PERSIST_DEBOUNCE_MS } from '@/lib/generate';

import { GenerationCard } from './index';

// Mock the service barrel (ExternalLink reaches for useOpenExternal) and the
// remove hook so the card renders without a provider tree. We capture the
// mutate spy to prove deletion only fires *after* confirmation.
const mockMutate = vi.fn();
const mockUpdateMutate = vi.fn();

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutate: vi.fn() }),
}));

vi.mock('@/services/use-ai-generations', () => ({
  useRemoveAiGeneration: () => ({ mutate: mockMutate, isPending: false }),
  // The card debounce-persists inline edits through this hook; the delete tests
  // don't edit, but the component calls it on render so the mock must provide it.
  useUpdateAiGeneration: () => ({ mutate: mockUpdateMutate, isPending: false }),
}));

// Translate to the raw key so assertions don't depend on locale resolution.
vi.mock('@/lib/i18n', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// Export helpers reach for Tauri IPC — stub them so tests stay offline.
vi.mock('@/lib/generate', async (importOriginal) => {
  const actual = await importOriginal<typeof Generate>();
  return {
    ...actual,
    exportPDF: vi.fn(),
    exportDOCX: vi.fn(),
    exportTXT: vi.fn(),
  };
});

const GEN: AiGenerationRecord = {
  id: 'gen-1',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  candidateName: 'Jane Doe',
  createdAt: Date.now(),
  mode: 'ats',
  board: '',
  jobUrl: '',
  resumeText: 'Original resume text.',
  coverLetterText: 'Original cover letter text.',
  jobAd: '',
  companyBrief: '',
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  targetLanguage: 'en',
  mismatch: false,
  topRequirements: [],
  applicationAnswers: [],
};

// ── Delete confirmation (pre-existing coverage) ───────────────────────────────

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

// ── Debounced persist (F1 edit-before-export) ─────────────────────────────────

describe('GenerationCard — debounced persist', () => {
  beforeEach(() => {
    mockUpdateMutate.mockClear();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /**
   * Open the resume section and switch EditableOutput to Edit mode so a
   * <textarea> is accessible. The section toggle is a `Button variant="unstyled"`
   * whose text content is the translation key for "Resume". EditableOutput opens
   * in Preview mode; the Edit radio must be clicked to surface the textarea.
   */
  function expandResumeAndSwitchToEdit() {
    // Click the section toggle whose visible text contains the resume key.
    const toggles = screen.queryAllByRole('button');
    const resumeToggle = toggles.find((b) =>
      (b.textContent ?? '').includes('resumes.generated.resume')
    );
    if (!resumeToggle) throw new Error('Resume section toggle not found');
    fireEvent.click(resumeToggle);

    // EditableOutput renders Preview by default; switch to Edit to get the textarea.
    fireEvent.click(screen.getByRole('radio', { name: /edit/i }));
  }

  /**
   * Open the cover-letter section and switch EditableOutput to Edit mode.
   * Symmetric to expandResumeAndSwitchToEdit but targets the coverLetter key.
   */
  function expandCoverAndSwitchToEdit() {
    const toggles = screen.queryAllByRole('button');
    const coverToggle = toggles.find((b) =>
      (b.textContent ?? '').includes('resumes.generated.coverLetter')
    );
    if (!coverToggle) throw new Error('Cover letter section toggle not found');
    fireEvent.click(coverToggle);

    fireEvent.click(screen.getByRole('radio', { name: /edit/i }));
  }

  it('editing the resume draft schedules a debounced update after PERSIST_DEBOUNCE_MS', () => {
    render(<GenerationCard gen={GEN} />);
    expandResumeAndSwitchToEdit();

    const textarea = screen.getByRole<HTMLTextAreaElement>('textbox');
    fireEvent.change(textarea, { target: { value: 'Edited resume text.' } });

    // Before the debounce fires: no call yet.
    expect(mockUpdateMutate).not.toHaveBeenCalled();

    // Advance past the debounce window — use the imported constant so this test
    // stays in lockstep with the production code's PERSIST_DEBOUNCE_MS value.
    act(() => {
      vi.advanceTimersByTime(PERSIST_DEBOUNCE_MS);
    });

    expect(mockUpdateMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateMutate).toHaveBeenCalledWith({
      id: 'gen-1',
      resumeText: 'Edited resume text.',
    });
  });

  it('rapid successive edits debounce to a single update call — proves timer cancellation', () => {
    render(<GenerationCard gen={GEN} />);
    expandResumeAndSwitchToEdit();

    const textarea = screen.getByRole<HTMLTextAreaElement>('textbox');

    // First edit starts the debounce timer.
    fireEvent.change(textarea, { target: { value: 'edit 1' } });

    // Advance to just before the debounce would fire — proves the first timer is
    // still pending and has NOT triggered a call yet.
    act(() => {
      vi.advanceTimersByTime(PERSIST_DEBOUNCE_MS - 1);
    });
    expect(mockUpdateMutate).not.toHaveBeenCalled();

    // Second edit cancels the first timer and starts a new one.
    fireEvent.change(textarea, { target: { value: 'edit 2' } });
    act(() => {
      vi.advanceTimersByTime(PERSIST_DEBOUNCE_MS - 1);
    });
    expect(mockUpdateMutate).not.toHaveBeenCalled();

    // Third (final) edit — again cancels the previous timer.
    fireEvent.change(textarea, { target: { value: 'edit 3' } });

    // Still no call before the debounce window for the third edit expires.
    expect(mockUpdateMutate).not.toHaveBeenCalled();

    // Now let the final debounce fully expire.
    act(() => {
      vi.advanceTimersByTime(PERSIST_DEBOUNCE_MS);
    });

    // Exactly one call, with the last value — no stale earlier values were flushed.
    expect(mockUpdateMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateMutate).toHaveBeenCalledWith({
      id: 'gen-1',
      resumeText: 'edit 3',
    });
  });

  it('editing the cover letter draft debounces update({ id, coverLetterText })', () => {
    render(<GenerationCard gen={GEN} />);
    expandCoverAndSwitchToEdit();

    const textarea = screen.getByRole<HTMLTextAreaElement>('textbox');
    fireEvent.change(textarea, { target: { value: 'Edited cover letter text.' } });

    // No call before the window closes.
    expect(mockUpdateMutate).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(PERSIST_DEBOUNCE_MS);
    });

    // Must use the coverLetterText key — not resumeText — proving the symmetric data path.
    expect(mockUpdateMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateMutate).toHaveBeenCalledWith({
      id: 'gen-1',
      coverLetterText: 'Edited cover letter text.',
    });
  });

  it('copy button reads the edited draft text, not the original gen prop', async () => {
    // Stub clipboard so the copy flow completes without a real Clipboard API.
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    render(<GenerationCard gen={GEN} />);
    expandResumeAndSwitchToEdit();

    const textarea = screen.getByRole<HTMLTextAreaElement>('textbox');
    fireEvent.change(textarea, { target: { value: 'Draft resume after edit.' } });

    // The copy button is always visible in the card header when draft has content.
    const copyBtn = screen.getByRole('button', {
      name: /resumes\.generated\.copyResume/i,
    });
    await act(async () => {
      fireEvent.click(copyBtn);
    });

    expect(writeText).toHaveBeenCalledWith('Draft resume after edit.');
  });
});
