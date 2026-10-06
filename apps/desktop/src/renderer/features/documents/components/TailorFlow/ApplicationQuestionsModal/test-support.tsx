/**
 * Shared fixtures + `vi.mock` factories for the ApplicationQuestionsModal tests.
 *
 * `vi.mock` calls are hoisted per test file, so each file keeps its own
 * one-line `vi.mock(..., async () => (await import('./test-support')).x)` and
 * this module owns the stubs they return.
 *
 * Heavy pieces (ModalShell focus trap, RewritePopover streaming) are stubbed so
 * the tests stay fast and deterministic. The real component logic (per-answer
 * rewriting state, disabled predicate, prop wiring) is exercised directly.
 */
import { type Mock, vi } from 'vitest';
import { fireEvent, screen } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

import type { ApplicationQuestionsModal } from '../ApplicationQuestionsModal';

export type PopoverProps = {
  target: { selection: string; before: string; after: string };
  docType: string;
  model: string;
  locale?: string;
  onAccept: (text: string) => void;
  onClose: () => void;
};

// Stubs the popover so we can drive onAccept / onClose without AI streaming.
export const RewritePopoverStub = vi.fn(({ target, docType, onAccept, onClose }: PopoverProps) => (
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

export const mockNotifyError = vi.fn() as Mock;

export const rewritePopoverModule = {
  RewritePopover: (props: PopoverProps) => RewritePopoverStub(props),
};

/** `@ajh/ui` with `ModalShell` flattened and `useNotification` wired to {@link mockNotifyError}. */
export const uiModule = (actual: typeof AjhUi): Record<string, unknown> => ({
  ...actual,
  ModalShell: ({ children, header }: { children: React.ReactNode; header: React.ReactNode }) => (
    <div>
      {header}
      {children}
    </div>
  ),
  useNotification: () => ({
    error: mockNotifyError,
    success: vi.fn() as Mock,
    info: vi.fn() as Mock,
  }),
});

/** `motion/react` with animations collapsed. */
export const motionModule = {
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: ({ children, ...rest }: React.HTMLAttributes<HTMLDivElement>) => (
      <div {...rest}>{children}</div>
    ),
  },
};

export const ANSWER_TEXT = 'Because I led a payments migration.';
export const QUESTION_ID = 'why-company';

export type UpdateAnswer = (id: string, text: string) => Promise<void>;
export type RevertAnswer = (id: string, prev: string) => void;

/** An `updateAnswer` spy whose save succeeds. */
export const savingUpdateAnswer = () => vi.fn<UpdateAnswer>().mockResolvedValue(undefined);

export function buildProps(
  overrides: Partial<React.ComponentProps<typeof ApplicationQuestionsModal>> = {}
) {
  return {
    selected: new Set<string>([QUESTION_ID]),
    toggle: vi.fn() as Mock,
    searchWeb: false,
    setSearchWeb: vi.fn() as Mock,
    custom: [],
    addCustom: vi.fn() as Mock,
    removeCustom: vi.fn() as Mock,
    answers: { [QUESTION_ID]: ANSWER_TEXT },
    generating: false,
    error: null,
    generate: vi.fn() as Mock,
    canGenerate: true,
    onClose: vi.fn() as Mock,
    model: 'llama3',
    locale: 'en',
    updateAnswer: savingUpdateAnswer(),
    revertAnswer: vi.fn<RevertAnswer>(),
    ...overrides,
  };
}

/** Clicks the (single) per-answer Rewrite button. */
export const clickRewrite = () =>
  fireEvent.click(
    screen.getByRole('button', { name: 'autopilot.apply.questions.rewriteAriaLabel' })
  );

/** Selects `substring` (must occur exactly once in `text`) inside `el`'s text
 *  node, mirroring a user drag-selecting part of the rendered answer. */
export function selectSubstring(el: HTMLElement, text: string, substring: string) {
  const textNode = el.firstChild as Text;
  const start = text.indexOf(substring);
  const range = document.createRange();
  range.setStart(textNode, start);
  range.setEnd(textNode, start + substring.length);
  const sel = window.getSelection();
  sel?.removeAllRanges();
  sel?.addRange(range);
}
