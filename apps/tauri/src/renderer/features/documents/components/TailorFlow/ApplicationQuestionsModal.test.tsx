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
 *
 * Heavy pieces (ModalShell focus trap, RewritePopover streaming) are stubbed so
 * this test stays fast and deterministic. The real component logic (per-answer
 * rewritingId state, disabled predicate, prop wiring) is exercised directly.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── RewritePopover stub ───────────────────────────────────────────────────────
// Stubs the popover so we can drive onAccept / onClose without AI streaming.

type PopoverProps = {
  target: { selection: string; before: string; after: string };
  docType: string;
  model: string;
  locale?: string;
  onAccept: (text: string) => void;
  onClose: () => void;
};

const RewritePopoverStub = vi.fn(({ target, docType, onAccept, onClose }: PopoverProps) => (
  <div data-testid="rewrite-popover" data-doc-type={docType} data-selection={target.selection}>
    <div
      role="button"
      tabIndex={0}
      onClick={() => onAccept('Rewritten answer text')}
      onKeyDown={() => onAccept('Rewritten answer text')}
      data-testid="popover-accept"
    >
      accept
    </div>
    <div
      role="button"
      tabIndex={0}
      onClick={onClose}
      onKeyDown={onClose}
      data-testid="popover-close"
    >
      cancel
    </div>
  </div>
));

vi.mock('@/components/generation/EditableOutput/RewritePopover', () => ({
  RewritePopover: (props: PopoverProps) => RewritePopoverStub(props),
}));

// ── @ajh/ui — stub ModalShell + useNotification, keep the rest ───────────────

const mockNotifyError = vi.fn();

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    ModalShell: ({ children, header }: { children: React.ReactNode; header: React.ReactNode }) => (
      <div>
        {header}
        {children}
      </div>
    ),
    useNotification: () => ({ error: mockNotifyError, success: vi.fn(), info: vi.fn() }),
  };
});

// ── motion/react — collapse animations ───────────────────────────────────────

vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: ({ children, ...rest }: React.HTMLAttributes<HTMLDivElement>) => (
      <div {...rest}>{children}</div>
    ),
  },
}));

// ── component under test ──────────────────────────────────────────────────────

import { ApplicationQuestionsModal } from './ApplicationQuestionsModal';

// ── fixtures ──────────────────────────────────────────────────────────────────

const ANSWER_TEXT = 'Because I led a payments migration.';
const QUESTION_ID = 'why-company';

function buildProps(
  overrides: Partial<React.ComponentProps<typeof ApplicationQuestionsModal>> = {}
) {
  return {
    selected: new Set<string>([QUESTION_ID]),
    toggle: vi.fn(),
    custom: [],
    addCustom: vi.fn(),
    removeCustom: vi.fn(),
    answers: { [QUESTION_ID]: ANSWER_TEXT },
    generating: false,
    error: null,
    generate: vi.fn(),
    canGenerate: true,
    onClose: vi.fn(),
    model: 'llama3',
    locale: 'en',
    updateAnswer: vi.fn<(id: string, text: string) => Promise<void>>().mockResolvedValue(undefined),
    ...overrides,
  };
}

beforeEach(() => {
  RewritePopoverStub.mockClear();
  mockNotifyError.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
});

// ── tests ─────────────────────────────────────────────────────────────────────

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

    fireEvent.click(
      screen.getByRole('button', { name: 'autopilot.apply.questions.rewriteAriaLabel' })
    );

    const popover = screen.getByTestId('rewrite-popover');
    expect(popover).toBeTruthy();
    expect(popover.getAttribute('data-doc-type')).toBe('application-answer');
    expect(popover.getAttribute('data-selection')).toBe(ANSWER_TEXT);
  });

  it('passes model and locale to the popover', () => {
    render(<ApplicationQuestionsModal {...buildProps({ model: 'gpt-4o', locale: 'de' })} />);
    fireEvent.click(
      screen.getByRole('button', { name: 'autopilot.apply.questions.rewriteAriaLabel' })
    );

    expect(RewritePopoverStub).toHaveBeenCalledWith(
      expect.objectContaining({ model: 'gpt-4o', locale: 'de' })
    );
  });

  it('onAccept calls updateAnswer with the question id and new text, then closes the popover', async () => {
    const updateAnswer = vi
      .fn<(id: string, text: string) => Promise<void>>()
      .mockResolvedValue(undefined);
    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer })} />);

    fireEvent.click(
      screen.getByRole('button', { name: 'autopilot.apply.questions.rewriteAriaLabel' })
    );
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
    const updateAnswer = vi
      .fn<(id: string, text: string) => Promise<void>>()
      .mockResolvedValue(undefined);
    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer })} />);

    fireEvent.click(
      screen.getByRole('button', { name: 'autopilot.apply.questions.rewriteAriaLabel' })
    );
    expect(screen.getByTestId('rewrite-popover')).toBeTruthy();

    fireEvent.click(screen.getByTestId('popover-close'));

    expect(screen.queryByTestId('rewrite-popover')).toBeNull();
    expect(updateAnswer).not.toHaveBeenCalled();
  });

  it('popover closes immediately even when updateAnswer rejects, and surfaces a fixed-key error toast', async () => {
    const updateAnswer = vi
      .fn<(id: string, text: string) => Promise<void>>()
      .mockRejectedValue(new Error('IPC save failed'));
    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer })} />);

    fireEvent.click(
      screen.getByRole('button', { name: 'autopilot.apply.questions.rewriteAriaLabel' })
    );
    expect(screen.getByTestId('rewrite-popover')).toBeTruthy();

    fireEvent.click(screen.getByTestId('popover-accept'));

    // Popover closes synchronously (before the promise settles).
    expect(screen.queryByTestId('rewrite-popover')).toBeNull();

    // Error toast is shown after the rejection settles.
    await waitFor(() => {
      expect(mockNotifyError).toHaveBeenCalledWith(
        expect.objectContaining({
          message: 'autopilot.apply.questions.rewriteSaveError',
        })
      );
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
