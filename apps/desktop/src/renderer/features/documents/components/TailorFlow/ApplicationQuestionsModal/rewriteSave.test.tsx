/**
 * ApplicationQuestionsModal — what happens when persisting an accepted rewrite
 * fails: the popover closes, the previous text is restored, an error toast
 * fires — and a superseded save's failure stays silent.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

import { ApplicationQuestionsModal } from '../ApplicationQuestionsModal';
import {
  ANSWER_TEXT,
  buildProps,
  clickRewrite,
  mockNotifyError,
  type PopoverProps,
  QUESTION_ID,
  type RevertAnswer,
  RewritePopoverStub,
  type UpdateAnswer,
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
  mockNotifyError.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
  window.getSelection()?.removeAllRanges();
});

/** Waits for the revert + error toast a failed save must produce. */
async function expectRevertAndToast(revertAnswer: ReturnType<typeof vi.fn<RevertAnswer>>) {
  await waitFor(() => {
    expect(revertAnswer).toHaveBeenCalledWith(QUESTION_ID, ANSWER_TEXT);
  });
  await waitFor(() => {
    expect(mockNotifyError).toHaveBeenCalledWith(
      expect.objectContaining({
        message: 'autopilot.apply.questions.rewriteSaveError',
      })
    );
  });
}

describe('ApplicationQuestionsModal — rewrite save failures', () => {
  it('on fast save failure: popover closes, revertAnswer restores previous text, error toast fires', async () => {
    // Reject immediately (before any React render cycle) — pendingRewriteRef is
    // set synchronously so the guard works even without a re-render.
    const updateAnswer = vi.fn<UpdateAnswer>().mockRejectedValue(new Error('IPC save failed'));
    const revertAnswer = vi.fn<RevertAnswer>();
    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer, revertAnswer })} />);

    clickRewrite();
    expect(screen.getByTestId('rewrite-popover')).toBeTruthy();

    // acceptRewrite fires — popover closes synchronously.
    await act(async () => {
      fireEvent.click(screen.getByTestId('popover-accept'));
    });
    expect(screen.queryByTestId('rewrite-popover')).toBeNull();

    await expectRevertAndToast(revertAnswer);
  });

  it('on deferred save failure: popover closes, revertAnswer restores previous text, error toast fires', async () => {
    // Deferred rejection — same assertions, just with a delayed reject.
    let rejectSave!: (e: Error) => void;
    const savePromise = new Promise<void>((_, rej) => {
      rejectSave = rej;
    });
    const updateAnswer = vi.fn<UpdateAnswer>(() => savePromise);
    const revertAnswer = vi.fn<RevertAnswer>();
    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer, revertAnswer })} />);

    clickRewrite();
    fireEvent.click(screen.getByTestId('popover-accept'));
    expect(screen.queryByTestId('rewrite-popover')).toBeNull();

    await act(async () => {
      rejectSave(new Error('IPC save failed'));
      await Promise.resolve();
    });

    await expectRevertAndToast(revertAnswer);
  });

  it('stale revert guard: if a second acceptRewrite supersedes A before A save fails, revertAnswer and the error toast are NOT triggered for A', async () => {
    // The shared stub always emits the SAME text, so A and B would be
    // indistinguishable. Override the stub for this test to emit different
    // text on successive accepts — this is the only way to prove the sentinel
    // correctly distinguishes "our text" from "a newer text".
    const REWRITE_A = 'Rewritten answer text — variant A';
    const REWRITE_B = 'Rewritten answer text — variant B';
    let stubCallCount = 0;
    RewritePopoverStub.mockImplementation(({ onAccept, onClose }: PopoverProps) => {
      const emitText = stubCallCount % 2 === 0 ? REWRITE_A : REWRITE_B;
      return (
        <div data-testid="rewrite-popover">
          <div
            role="button"
            tabIndex={0}
            onClick={() => onAccept(emitText)}
            onKeyDown={() => onAccept(emitText)}
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
      );
    });

    let rejectA!: (e: Error) => void;
    const saveAPromise = new Promise<void>((_, rej) => {
      rejectA = rej;
    });
    // First call → pending saveA (emits textA); second call → resolve immediately (emits textB).
    let callCount = 0;
    const updateAnswer = vi.fn<UpdateAnswer>(() => {
      callCount += 1;
      return callCount === 1 ? saveAPromise : Promise.resolve();
    });
    const revertAnswer = vi.fn<RevertAnswer>();

    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer, revertAnswer })} />);

    // Accept rewrite A — pendingRewriteRef.current[QUESTION_ID] = REWRITE_A.
    clickRewrite();
    fireEvent.click(screen.getByTestId('popover-accept'));
    stubCallCount += 1; // next open will emit REWRITE_B

    // Accept rewrite B — pendingRewriteRef.current[QUESTION_ID] overwritten to REWRITE_B.
    // (Re-open the popover — acceptRewrite closed it.)
    clickRewrite();
    await act(async () => {
      fireEvent.click(screen.getByTestId('popover-accept'));
    });

    // Now let save A reject — guard: pendingRewriteRef.current[id] === REWRITE_B ≠ REWRITE_A → skip revert.
    await act(async () => {
      rejectA(new Error('IPC save failed'));
      await Promise.resolve();
    });

    // revertAnswer must NOT have been called — B superseded A.
    expect(revertAnswer).not.toHaveBeenCalled();
    // Nor should the error toast fire — B is the current, already-saved
    // answer; surfacing "save failed" for A's superseded rejection would be a
    // stale, misleading toast for text the user no longer sees.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0)); // flush the reject→catch microtask chain
    });
    expect(mockNotifyError).not.toHaveBeenCalled();
  });

  it('still-current save failure DOES toast even with a stale pendingRewriteRef entry from an earlier accept', async () => {
    // First accept succeeds and is cleared from pendingRewriteRef (fix 2); a
    // second, still-current accept then fails and must still surface a toast.
    const updateAnswer = vi
      .fn<UpdateAnswer>()
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error('IPC save failed'));
    const revertAnswer = vi.fn<RevertAnswer>();
    render(<ApplicationQuestionsModal {...buildProps({ updateAnswer, revertAnswer })} />);

    // First accept — succeeds, clearing the sentinel.
    clickRewrite();
    await act(async () => {
      fireEvent.click(screen.getByTestId('popover-accept'));
    });
    expect(mockNotifyError).not.toHaveBeenCalled();

    // Second accept — this save fails and is still the current one.
    clickRewrite();
    await act(async () => {
      fireEvent.click(screen.getByTestId('popover-accept'));
    });

    await waitFor(() => {
      expect(mockNotifyError).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'autopilot.apply.questions.rewriteSaveError' })
      );
    });
    expect(revertAnswer).toHaveBeenCalledWith(QUESTION_ID, ANSWER_TEXT);
  });
});
