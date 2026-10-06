/**
 * ApplicationQuestionsModal — Rewrite-with-AI integration tests.
 *
 * Covers:
 *  - Rewrite button renders per answer (not before an answer exists).
 *  - Clicking Rewrite opens RewritePopover with docType='application-answer'
 *    and the full answer text as the selection.
 *  - Only one popover is open at a time (opening a second closes the first).
 *  - onAccept calls updateAnswer with the question id + new text, then closes.
 *  - onClose (ESC / Cancel) clears the popover without updating the answer.
 *  - Copy button still present and independent of Rewrite.
 *  - The opt-in web-search toggle.
 *
 * Save-failure handling lives in `rewriteSave.test.tsx`.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

import { ApplicationQuestionsModal } from '../ApplicationQuestionsModal';
import {
  ANSWER_TEXT,
  buildProps,
  clickRewrite,
  QUESTION_ID,
  RewritePopoverStub,
  savingUpdateAnswer,
  selectSubstring,
} from './test-support';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
vi.mock('@/components/generation/EditableOutput/RewritePopover', async () => {
  return (await import('./test-support')).rewritePopoverModule;
});
vi.mock('@ajh/ui', async (importOriginal) => {
  return (await import('./test-support')).uiModule(await importOriginal<typeof AjhUi>());
});
vi.mock('motion/react', async () => (await import('./test-support')).motionModule);

beforeEach(() => {
  RewritePopoverStub.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
  // Selections persist on the jsdom `window` across tests in the same file.
  window.getSelection()?.removeAllRanges();
});

describe('ApplicationQuestionsModal — Rewrite with AI', () => {
  it('renders a Rewrite button for each answer that has text', () => {
    render(<ApplicationQuestionsModal {...buildProps()} />);
    const rewriteBtn = screen.getByRole('button', {
      name: 'autopilot.apply.questions.rewriteAriaLabel',
    });
    expect(rewriteBtn).toBeTruthy();
  });

  it('does NOT render a Rewrite button for a question with no answer yet', () => {
    render(<ApplicationQuestionsModal {...buildProps({ answers: {} })} />);
    expect(
      screen.queryByRole('button', { name: 'autopilot.apply.questions.rewriteAriaLabel' })
    ).toBeNull();
  });

  it('clicking Rewrite opens the popover with docType=application-answer and the answer as selection', () => {
    render(<ApplicationQuestionsModal {...buildProps()} />);
    expect(screen.queryByTestId('rewrite-popover')).toBeNull();

    clickRewrite();

    const popover = screen.getByTestId('rewrite-popover');
    expect(popover).toBeTruthy();
    expect(popover.getAttribute('data-doc-type')).toBe('application-answer');
    expect(popover.getAttribute('data-selection')).toBe(ANSWER_TEXT);
  });

  it('selecting part of the answer targets only that substring for rewrite', () => {
    render(<ApplicationQuestionsModal {...buildProps()} />);
    selectSubstring(screen.getByText(ANSWER_TEXT), ANSWER_TEXT, 'led a payments migration');

    clickRewrite();

    expect(screen.getByTestId('rewrite-popover').getAttribute('data-selection')).toBe(
      'led a payments migration'
    );
  });

  it('accepting a rewrite of a selected substring splices it back into the full answer', async () => {
    const updateAnswer = savingUpdateAnswer();
    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer })} />);
    selectSubstring(screen.getByText(ANSWER_TEXT), ANSWER_TEXT, 'led a payments migration');

    clickRewrite();
    fireEvent.click(screen.getByTestId('popover-accept')); // stub emits 'Rewritten answer text'

    await waitFor(() => {
      expect(updateAnswer).toHaveBeenCalledWith(QUESTION_ID, 'Because I Rewritten answer text.');
    });
  });

  it('passes model and locale to the popover', () => {
    render(<ApplicationQuestionsModal {...buildProps({ model: 'gpt-4o', locale: 'de' })} />);
    clickRewrite();

    expect(RewritePopoverStub).toHaveBeenCalledWith(
      expect.objectContaining({ model: 'gpt-4o', locale: 'de' })
    );
  });

  it('onAccept calls updateAnswer with the question id and new text, then closes the popover', async () => {
    const updateAnswer = savingUpdateAnswer();
    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer })} />);

    clickRewrite();
    fireEvent.click(screen.getByTestId('popover-accept'));

    await waitFor(() => {
      expect(updateAnswer).toHaveBeenCalledWith(QUESTION_ID, 'Rewritten answer text');
    });
    // Popover closes after accept
    await waitFor(() => {
      expect(screen.queryByTestId('rewrite-popover')).toBeNull();
    });
  });

  it('onClose dismisses the popover without calling updateAnswer', () => {
    const updateAnswer = savingUpdateAnswer();
    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer })} />);

    clickRewrite();
    expect(screen.getByTestId('rewrite-popover')).toBeTruthy();

    fireEvent.click(screen.getByTestId('popover-close'));

    expect(screen.queryByTestId('rewrite-popover')).toBeNull();
    expect(updateAnswer).not.toHaveBeenCalled();
  });

  describe('opt-in web-search toggle', () => {
    it('renders off by default and toggles on click', () => {
      const setSearchWeb = vi.fn();
      render(<ApplicationQuestionsModal {...buildProps({ setSearchWeb })} />);

      const toggle = screen.getByRole('switch', {
        name: 'autopilot.apply.questions.searchWeb.label',
      });
      expect(toggle.getAttribute('aria-checked')).toBe('false');

      fireEvent.click(toggle);
      expect(setSearchWeb).toHaveBeenCalledWith(true);
    });

    it('reflects an already-on state', () => {
      render(<ApplicationQuestionsModal {...buildProps({ searchWeb: true })} />);
      const toggle = screen.getByRole('switch', {
        name: 'autopilot.apply.questions.searchWeb.label',
      });
      expect(toggle.getAttribute('aria-checked')).toBe('true');
    });

    it('is disabled while generating, so it cannot be toggled mid-generation', () => {
      const setSearchWeb = vi.fn();
      render(<ApplicationQuestionsModal {...buildProps({ generating: true, setSearchWeb })} />);
      const toggle = screen.getByRole('switch', {
        name: 'autopilot.apply.questions.searchWeb.label',
      });
      expect(toggle).toBeDisabled();

      fireEvent.click(toggle);
      expect(setSearchWeb).not.toHaveBeenCalled();
    });
  });

  it('Copy button is still present alongside Rewrite', () => {
    render(<ApplicationQuestionsModal {...buildProps()} />);
    expect(screen.getByRole('button', { name: 'autopilot.apply.questions.copy' })).toBeTruthy();
    expect(
      screen.getByRole('button', { name: 'autopilot.apply.questions.rewriteAriaLabel' })
    ).toBeTruthy();
  });

  it('opening a second Rewrite closes the first (only one popover at a time)', () => {
    const secondId = 'why-role';
    const secondAnswer = 'Because the role matches my skills.';
    render(
      <ApplicationQuestionsModal
        {...buildProps({
          answers: {
            [QUESTION_ID]: ANSWER_TEXT,
            [secondId]: secondAnswer,
          },
        })}
      />
    );

    const rewriteBtns = screen.getAllByRole('button', {
      name: 'autopilot.apply.questions.rewriteAriaLabel',
    });
    expect(rewriteBtns).toHaveLength(2);

    // Open first
    fireEvent.click(rewriteBtns[0] as HTMLElement);
    expect(screen.getAllByTestId('rewrite-popover')).toHaveLength(1);

    // Open second — first should close
    fireEvent.click(rewriteBtns[1] as HTMLElement);
    expect(screen.getAllByTestId('rewrite-popover')).toHaveLength(1);
    const popover = screen.getByTestId('rewrite-popover');
    expect(popover.getAttribute('data-selection')).toBe(secondAnswer);
  });
});
