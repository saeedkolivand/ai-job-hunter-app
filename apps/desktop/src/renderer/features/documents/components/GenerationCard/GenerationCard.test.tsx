import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, within } from '@testing-library/react';

import type * as AjhTranslations from '@ajh/translations';

import { PERSIST_DEBOUNCE_MS } from '@/lib/generate';

import { GEN, GenerationCard, mockMutate, mockUpdateMutate, translator } from './test-render';

// ── Delete confirmation (pre-existing coverage) ───────────────────────────────

describe('GenerationCard — delete confirmation', () => {
  beforeEach(() => mockMutate.mockClear());

  const DELETE = 'resumes.generated.delete';
  const ACTIONS = 'resumes.generated.actions';

  /** Delete now lives in the 3-dots overflow menu (#32): open it, click Delete. */
  function openMenuClickDelete() {
    fireEvent.click(screen.getByRole('button', { name: ACTIONS }));
    fireEvent.click(screen.getByRole('menuitem', { name: DELETE }));
  }

  it('does not delete until the confirm dialog is accepted', () => {
    render(<GenerationCard gen={GEN} />);

    openMenuClickDelete();

    // Choosing Delete from the menu must NOT delete immediately — it opens the
    // confirm dialog first (that was the data-loss bug).
    expect(mockMutate).not.toHaveBeenCalled();

    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getByText('resumes.generated.deleteTitle')).toBeInTheDocument();

    // Accept inside the dialog (scoped so we don't match the menu item).
    fireEvent.click(within(dialog).getByRole('button', { name: DELETE }));
    expect(mockMutate).toHaveBeenCalledTimes(1);
    expect(mockMutate).toHaveBeenCalledWith('gen-1');
  });

  it('dismisses without deleting when cancelled', () => {
    render(<GenerationCard gen={GEN} />);

    openMenuClickDelete();
    const dialog = screen.getByRole('dialog');
    // ConfirmModal's cancel label is a literal default ("Cancel"), not translated.
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));

    expect(mockMutate).not.toHaveBeenCalled();
  });
});

// ── Board chip labeling (ADR-031 follow-up) ───────────────────────────────────

// These render through the REAL en resources (not the identity mock) so they
// verify the actual localized strings — a missing or mistranslated `jobs.boards.*`
// entry fails here, which the raw-key assertions could not catch.
describe('GenerationCard — board chip', () => {
  beforeAll(async () => {
    const actual = await vi.importActual<typeof AjhTranslations>('@ajh/translations');
    const realT = actual.default.getFixedT('en');
    translator.current = (key, opts) => String(realT(key, opts ?? {}));
  });

  afterAll(() => {
    // Restore identity so the debounced-persist block keeps asserting on raw keys.
    translator.current = (key) => key;
  });

  it('labels a known board with its localized name (jobs.boards.linkedin → "LinkedIn")', () => {
    render(<GenerationCard gen={{ ...GEN, board: 'linkedin' }} />);
    expect(screen.getByText('LinkedIn')).toBeInTheDocument();
  });

  it('labels a URL-imported generation with the localized web-import name (jobs.boards.url → "Web import")', () => {
    render(<GenerationCard gen={{ ...GEN, board: 'url' }} />);
    expect(screen.getByText('Web import')).toBeInTheDocument();
    // The raw source id is never rendered as the badge.
    expect(screen.queryByText('url')).not.toBeInTheDocument();
  });

  it('renders the raw board id when no jobs.boards.<id> resource exists (defaultValue fallback)', () => {
    render(<GenerationCard gen={{ ...GEN, board: 'totally-unknown-board' }} />);
    expect(screen.getByText('totally-unknown-board')).toBeInTheDocument();
  });

  it('renders no board chip when the generation has no board (pasted text)', () => {
    const { container } = render(<GenerationCard gen={{ ...GEN, board: '', jobUrl: '' }} />);
    // Only the always-present mode chip occupies the uppercase-chip row — the
    // empty board yields no chip (not an empty one).
    expect(container.querySelectorAll('.uppercase')).toHaveLength(1);
    expect(screen.getByText('ats')).toBeInTheDocument();
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

  /** The card collapses to its header row (#27); reveal the body first. */
  function expandCard() {
    fireEvent.click(screen.getByRole('button', { name: /Senior Engineer/ }));
  }

  /**
   * Open the resume section and switch EditableOutput to Source mode so the raw
   * <textarea> is accessible. The section toggle is a `Button variant="unstyled"`
   * whose text content is the translation key for "Resume". EditableOutput opens
   * in Preview mode; the Source radio must be clicked to surface the raw textarea
   * (Edit is now the WYSIWYG RichTextEditor surface).
   */
  function expandResumeAndSwitchToEdit() {
    expandCard();
    // Click the section toggle whose visible text contains the resume key.
    const toggles = screen.queryAllByRole('button');
    const resumeToggle = toggles.find((b) =>
      (b.textContent ?? '').includes('resumes.generated.resume')
    );
    if (!resumeToggle) throw new Error('Resume section toggle not found');
    fireEvent.click(resumeToggle);

    // EditableOutput renders Preview by default; switch to Source to get the raw textarea
    // (Edit is now the WYSIWYG surface; t() returns raw keys so name is 'aiGenerate.source').
    fireEvent.click(screen.getByRole('radio', { name: /aiGenerate\.source/i }));
  }

  /**
   * Open the cover-letter section and switch EditableOutput to Source mode.
   * Symmetric to expandResumeAndSwitchToEdit but targets the coverLetter key.
   */
  function expandCoverAndSwitchToEdit() {
    expandCard();
    const toggles = screen.queryAllByRole('button');
    const coverToggle = toggles.find((b) =>
      (b.textContent ?? '').includes('resumes.generated.coverLetter')
    );
    if (!coverToggle) throw new Error('Cover letter section toggle not found');
    fireEvent.click(coverToggle);

    fireEvent.click(screen.getByRole('radio', { name: /aiGenerate\.source/i }));
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
});
