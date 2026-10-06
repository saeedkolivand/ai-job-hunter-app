/**
 * TailorFlow — layout + a11y contracts: height chain, cancelled hint (H9), focus follows the stage (M6/N2), live-region announcer (CR-7).
 * Shared mocks, state and helpers live in `test-support.tsx` (see its header).
 */
import { beforeEach, describe, expect, it, onTestFinished } from 'vitest';
import { screen } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import { genMock, renderFlow, rerenderFlow, resetState } from './TailorFlow.test-support';

beforeEach(resetState);

const {
  resultsPanel,
  tailorWizard,
  generatingPanel,
  generationCancelled,
  generationError,
  liveAnnouncer,
} = TEST_IDS.documents;
const announcer = () => screen.getByTestId(liveAnnouncer);

// ─────────────────────────────────────────────────────────────────────────────
// 6. Height chain (load-bearing layout)
// ─────────────────────────────────────────────────────────────────────────────
// GenerationOutput pins its header by being height-bounded, which only works if
// every ancestor passes a bounded height down. These two links are that chain's
// top: drop either and the viewer grows past the window again, an ancestor
// becomes the scroll owner and the header scrolls away with the document — while
// every assertion inside GenerationOutput/ResultsPanel stays green (they only
// walk up to their own render container).

describe('TailorFlow — height chain', () => {
  it('bounds the stage body and stretches the stage to it, on every stage', () => {
    for (const stage of ['configuring', 'done'] as const) {
      genMock.hasOutput = stage === 'done';

      const { unmount } = renderFlow({});
      const testId = stage === 'done' ? resultsPanel : tailorWizard;

      // Stage element (motion.div) must fill the stage body…
      const stageEl = screen.getByTestId(testId).parentElement;
      expect(stageEl, stage).not.toBeNull();
      expect(stageEl?.className, stage).toContain('h-full');

      // …and the stage body must be a bounded flex child, never content-sized.
      const stageBody = stageEl?.parentElement;
      expect(stageBody, stage).not.toBeNull();
      expect(stageBody?.className, stage).toContain('min-h-0');
      expect(stageBody?.className, stage).toContain('flex-1');

      // The root the two hang off is itself height-bounded.
      const root = stageBody?.parentElement;
      expect(root?.className, stage).toContain('h-full');
      expect(root?.className, stage).toContain('min-h-0');

      unmount();
    }
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 7. Cancelled-with-no-output acknowledgement (H9)
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — cancelled-before-any-output hint (H9)', () => {
  it('shows an acknowledgement on the configuring stage when cancelled with no output', () => {
    genMock.state = 'cancelled';
    genMock.busy = false;
    genMock.hasOutput = false;
    renderFlow({});
    expect(screen.getByTestId(generationCancelled)).toHaveTextContent(
      'autopilot.apply.cancelledNoOutput'
    );
  });

  it('a start failure (gen.error) takes priority over the cancelled hint', () => {
    genMock.state = 'cancelled';
    genMock.error = 'Model timed out';
    renderFlow({});
    expect(screen.getByTestId(generationError)).toBeInTheDocument();
    expect(screen.queryByTestId(generationCancelled)).not.toBeInTheDocument();
  });

  it.each([
    ['does not show it while idle (nothing to acknowledge)', { state: 'idle' }],
    [
      'does not show once output exists (done stage) even if state is still cancelled',
      { state: 'cancelled', hasOutput: true },
    ],
  ])('%s', (_name, state) => {
    Object.assign(genMock, state);
    renderFlow({});
    expect(screen.queryByTestId(generationCancelled)).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 8. Focus follows the stage — but never STEALS it from mount or a modal (M6/N2)
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — focus follows the stage (M6/N2)', () => {
  it('does NOT steal focus on mount (only on a subsequent stage CHANGE)', () => {
    renderFlow({});
    // Nothing focused this render — the effect's mount guard must no-op.
    expect(screen.getByTestId(tailorWizard).parentElement).not.toHaveFocus();
    expect(document.activeElement === document.body).toBe(true);
  });

  it('focuses the (inert) stage body element on a stage CHANGE after mount', () => {
    const { rerender } = renderFlow({});
    genMock.busy = true;
    rerender(rerenderFlow());
    expect(screen.getByTestId(generatingPanel).parentElement).toHaveFocus();

    genMock.busy = false;
    genMock.hasOutput = true;
    rerender(rerenderFlow());
    expect(screen.getByTestId(resultsPanel).parentElement).toHaveFocus();
  });

  // N2: Interview-questions/Referral stay open (and enabled) while a run is
  // busy (ApplicationDetailPage's toolbar) — a stage flip mid-run must not
  // pull focus out from under an open dialog. `useFocusTrap` only intercepts
  // Tab, so a stray programmatic `.focus()` landing outside the trap is not
  // caught by anything else.
  it('does NOT steal focus from an element inside an open modal (aria-modal) on a stage change', () => {
    const { rerender } = renderFlow({});

    const dialog = document.createElement('div');
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    const dialogButton = document.createElement('button');
    dialog.appendChild(dialogButton);
    document.body.appendChild(dialog);
    // CR-8: registered via `onTestFinished`, not a trailing statement — a
    // failed assertion above would otherwise skip this cleanup and leave a
    // `[aria-modal="true"]` node in `document.body` for every LATER test in
    // this file, which could silently suppress the N2 focus guard in a way
    // that only reproduces depending on run order.
    onTestFinished(() => dialog.remove());
    dialogButton.focus();
    expect(dialogButton).toHaveFocus();

    genMock.busy = true;
    rerender(rerenderFlow());

    expect(dialogButton).toHaveFocus();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 9. Persistent live-region announcer (CR-7)
// ─────────────────────────────────────────────────────────────────────────────
// Several screen readers do not announce content added to the a11y tree in
// the SAME update that creates the region — a `role="status"` div that only
// mounts once its condition is already true is unreliable. This region is
// mounted for TailorFlow's entire lifetime; only its text changes.

describe('TailorFlow — persistent live-region announcer (CR-7)', () => {
  it('is present on mount, before there is anything to announce', () => {
    renderFlow({});
    expect(announcer()).toBeInTheDocument();
  });

  it('announces the cancelled-no-output state', () => {
    genMock.state = 'cancelled';
    renderFlow({});
    expect(announcer()).toHaveTextContent('autopilot.apply.cancelledNoOutput');
  });

  it('announces needsReview once the done stage renders with that status', () => {
    genMock.state = 'needsReview';
    genMock.hasOutput = true;
    renderFlow({});
    expect(announcer()).toHaveTextContent('pipeline.status.needsReview');
  });

  // CR-10: without clearing the region on the null transition, a run that
  // finishes cleanly after an earlier cancel kept exposing the stale
  // "cancelled" text forever (the region is mounted for the whole component
  // lifetime, so nothing else ever overwrote it).
  it('clears the live region once the cancelled state ends', () => {
    genMock.state = 'cancelled';
    const { rerender } = renderFlow({});
    expect(announcer()).toHaveTextContent('autopilot.apply.cancelledNoOutput');

    genMock.state = 'idle';
    genMock.busy = true;
    rerender(rerenderFlow());

    expect(announcer()).toBeEmptyDOMElement();
  });
});
