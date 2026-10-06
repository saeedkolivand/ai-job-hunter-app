/**
 * TailorFlow — controller seam, job-ad resolution (prefer-longer), host job-desc callback, Score-tab résumé id.
 * Shared mocks, state and helpers live in `test-support.tsx` (see its header).
 */
import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import type { TailorFlowController } from './index';
import {
  answersMock,
  docsState,
  genMock,
  JOB,
  makePersistence,
  renderFlow,
  resetState,
  resolveJobUrlState,
} from './TailorFlow.test-support';

beforeEach(resetState);

// ─────────────────────────────────────────────────────────────────────────────
// 3. Controller seam — onController shape + modal triggers
// ─────────────────────────────────────────────────────────────────────────────

/** Renders and returns a getter for the controller most recently reported to the host. */
function renderWithController() {
  const onController = vi.fn();
  renderFlow({ onController });
  return () => onController.mock.calls.at(-1)?.[0] as TailorFlowController | undefined;
}

describe('TailorFlow — controller seam', () => {
  it('calls onController with stage=configuring when no output and not busy', () => {
    const controller = renderWithController()();
    expect(controller).toBeDefined();
    expect(controller?.stage).toBe('configuring');
  });

  it.each([
    ['generating', 'when busy=true', { busy: true }],
    ['done', 'when output exists and not busy', { hasOutput: true }],
  ])('calls onController with stage=%s %s', (stage, _when, state) => {
    Object.assign(genMock, state);
    expect(renderWithController()()?.stage).toBe(stage);
  });

  it.each([
    ['reports questionsCount=0 when selected is empty', [], 0],
    ['reports questionsCount reflecting selected.size', ['q1', 'q2', 'q3'], 3],
  ])('%s', (_name, selected, count) => {
    answersMock.selected = new Set<string>(selected);
    expect(renderWithController()()?.questionsCount).toBe(count);
  });

  it('controller exposes openQuestions and openReferral as functions', () => {
    const controller = renderWithController()();
    expect(typeof controller?.openQuestions).toBe('function');
    expect(typeof controller?.openReferral).toBe('function');
  });

  const MODALS = [
    [
      'openQuestions',
      'ApplicationQuestionsModal',
      TEST_IDS.documents.questionsModal,
      'close-questions',
    ],
    ['openReferral', 'ReferralModal', TEST_IDS.documents.referralModal, 'close-referral'],
  ] as const;

  it.each(MODALS)('calling %s() opens the %s', async (open, _modal, testId) => {
    const controller = renderWithController();
    expect(screen.queryByTestId(testId)).not.toBeInTheDocument();

    // Wrap the imperative state-update in act() so React flushes synchronously.
    act(() => {
      controller()?.[open]();
    });

    expect(await screen.findByTestId(testId)).toBeInTheDocument();
  });

  it.each(MODALS)(
    'closing the modal opened by %s() (%s) removes it from the DOM',
    async (open, _modal, testId, closeName) => {
      const user = userEvent.setup();
      const controller = renderWithController();

      act(() => {
        controller()?.[open]();
      });
      expect(await screen.findByTestId(testId)).toBeInTheDocument();

      await user.click(screen.getByRole('button', { name: closeName }));
      expect(screen.queryByTestId(testId)).not.toBeInTheDocument();
    }
  );

  it('onController is not required — component renders without it', () => {
    // Verify no crash when onController prop is omitted.
    expect(() => renderFlow({})).not.toThrow();
    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4. prefer-longer / skip-refetch branch (SHORT_DESC_FLOOR = 800)
// ─────────────────────────────────────────────────────────────────────────────

// A string of exactly `n` 'x' characters — avoids import of a pad utility.
const repeat = (n: number) => 'x'.repeat(n);

describe('TailorFlow — prefer-longer / useResolveJobUrl branch', () => {
  it('(a) short initialDesc + longer fetchedDesc → fetchedDesc wins (forwarded to TailorWizard)', () => {
    // initialDesc is 10 chars (< 800): re-resolve is triggered.
    // fetchedDesc is 900 chars: longer than initialDesc → must win.
    const shortDesc = repeat(10);
    const longFetched = repeat(900);
    resolveJobUrlState.data = { description: longFetched };

    renderFlow({
      job: { ...JOB, description: shortDesc },
    });

    // jobDesc flowed into TailorWizard as the jobDesc prop → exposed as data-jobdesc.
    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toHaveAttribute(
      'data-jobdesc',
      longFetched
    );
  });

  it('(b) long initialDesc (≥800) → useResolveJobUrl called with shouldFetch=false', () => {
    // initialDesc is 800 chars: at the floor, re-resolve is skipped.
    const longDesc = repeat(800);

    renderFlow({
      job: { ...JOB, description: longDesc },
    });

    // The 2nd arg to useResolveJobUrl must be false when initialDesc.length >= SHORT_DESC_FLOOR.
    expect(resolveJobUrlState.lastShouldFetch).toBe(false);
  });

  it('(c) equal-length fetchedDesc and initialDesc → initialDesc (carried) wins', () => {
    // Both are 50 chars: fetchedDesc.length > initialDesc.length is false → initialDesc wins.
    const carried = repeat(50);
    const fetched = repeat(50);
    resolveJobUrlState.data = { description: fetched };

    renderFlow({
      job: { ...JOB, description: carried },
    });

    // jobDesc must equal the carried initialDesc, not the fetched one.
    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toHaveAttribute(
      'data-jobdesc',
      carried
    );
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5. onJobDescChange prop — host persist callback
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — onJobDescChange host callback', () => {
  it('calls onJobDescChange when the user edits the job ad in the configuring stage', async () => {
    // The TailorWizard stub exposes an "edit-jobdesc" button that calls
    // onJobDescChange('edited-job-ad'). TailorFlow must forward this to the host
    // via the new prop, in addition to updating its internal jobDescOverride.
    const onJobDescChange = vi.fn();
    const user = userEvent.setup();
    renderFlow({ onJobDescChange });

    await user.click(screen.getByTestId('wizard-edit-jobdesc'));

    expect(onJobDescChange).toHaveBeenCalledTimes(1);
    expect(onJobDescChange).toHaveBeenCalledWith('edited-job-ad');
  });

  it('does NOT throw when onJobDescChange is omitted (autopilot callers unaffected)', async () => {
    // Omitting the prop must not throw — the optional-call guard `onJobDescChange?.()` covers it.
    const user = userEvent.setup();
    expect(() => renderFlow({})).not.toThrow();

    await expect(user.click(screen.getByTestId('wizard-edit-jobdesc'))).resolves.not.toThrow();
  });

  it('still updates the internal jobDesc (forwarded to TailorWizard) even without the host prop', async () => {
    // Editing with no onJobDescChange still updates jobDescOverride so the
    // job ad textarea reflects the user's paste in the wizard.
    const user = userEvent.setup();
    renderFlow({});

    await user.click(screen.getByTestId('wizard-edit-jobdesc'));

    // After the edit, jobDesc is 'edited-job-ad' — forwarded to the wizard as data-jobdesc.
    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toHaveAttribute(
      'data-jobdesc',
      'edited-job-ad'
    );
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Score-tab résumé id — the SAVED résumé, which is not always the form's field
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — which résumé the Score tab scores', () => {
  it('falls back to the default saved résumé when the form has no picked document', () => {
    // The autopilot apply path seeds `ap.resumeText`, a snapshot that can differ
    // from the document it came from, so `resumeDocId` stays deliberately unset —
    // which used to leave the Score tab permanently on "Save a résumé to score".
    docsState.docs = [{ _id: 'doc-default', name: 'Resume.pdf', isDefault: true }];

    renderFlow({});

    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toHaveAttribute(
      'data-resumeid',
      'doc-default'
    );
  });

  it('prefers an explicitly picked document over the default', () => {
    docsState.docs = [
      { _id: 'doc-default', name: 'Resume.pdf', isDefault: true },
      { _id: 'doc-picked', name: 'Other.pdf' },
    ];
    const persistence = makePersistence();
    persistence.wizardForm = {
      resume: 'My resume',
      outputType: 'both',
      researchCompany: false,
      resumeDocId: 'doc-picked',
    };

    renderFlow({ persistence });

    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toHaveAttribute(
      'data-resumeid',
      'doc-picked'
    );
  });

  it('scores nothing when there is no saved résumé at all — never a fabricated id', () => {
    docsState.docs = [];

    renderFlow({});

    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toHaveAttribute(
      'data-resumeid',
      ''
    );
  });
});
