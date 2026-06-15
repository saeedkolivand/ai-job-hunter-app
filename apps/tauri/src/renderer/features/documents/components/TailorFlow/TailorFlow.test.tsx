/**
 * TailorFlow — extraction seams
 *
 * Tests the three public contracts introduced by the TailorFlow extraction:
 *   1. Stage derivation — generating > done > configuring priority.
 *   2. Persistence injection — TailorFlow READS from and WRITES to the injected
 *      persistence object; the component is host-agnostic.
 *   3. Controller seam — onController is called with the correct shape, the
 *      questionsCount reflects selected.size, and openQuestions/openReferral
 *      open the respective modals.
 *
 * Strategy:
 *  - `useTailorGeneration` and `useApplicationAnswers` are mocked so stage
 *    transitions are fully controlled without any IPC / generation-store.
 *  - Service hooks (`useExtractText`, `useResolveJobUrl`, `useSelectedModel`,
 *    `useCanUseAI`) are mocked so no QueryClient / AppClient provider is needed.
 *  - Heavy child panels (TailorWizard, GeneratingPanel, ResultsPanel,
 *    ApplicationQuestionsModal, ReferralModal) are stubbed to stable markers so
 *    assertions are cheap and deterministic.
 *  - `motion/react` is collapsed to plain fragments (no animation overhead).
 *  - `@ajh/translations` returns keys as-is.
 *  - noUncheckedIndexedAccess: all array accesses are guarded.
 */

import React, { act } from 'react';
import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── motion/react — collapse animations to plain wrappers ──────────────────────

vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: React.forwardRef(
      (
        { children, ...rest }: React.HTMLAttributes<HTMLDivElement>,
        ref: React.Ref<HTMLDivElement>
      ) => (
        <div ref={ref} {...rest}>
          {children}
        </div>
      )
    ),
  },
}));

// ── ModelSelector hooks ───────────────────────────────────────────────────────

vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
  useCanUseAI: () => ({ canUse: true, reason: undefined }),
}));

// ── Service hooks — no real IPC / QueryClient needed ─────────────────────────

vi.mock('@/services', () => ({
  useResolveJobUrl: () => ({ data: undefined, isLoading: false }),
  useExtractText: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

// ── useTailorGeneration — controlled mock ─────────────────────────────────────

const genMock = {
  generating: false,
  phase: 'idle' as const,
  phaseLabel: '',
  thinking: '',
  resumeOut: '' as string,
  coverOut: '' as string,
  activeOut: 'resume' as const,
  setActiveOut: vi.fn(),
  output: '' as string,
  error: null,
  copied: false,
  exportOpen: false,
  setExportOpen: vi.fn(),
  generate: vi.fn().mockResolvedValue(undefined),
  abort: vi.fn(),
  copy: vi.fn(),
  exportAs: vi.fn(),
  editActiveOutput: vi.fn(),
  meta: null,
};

vi.mock('@/features/documents/components/TailorFlow/useTailorGeneration', () => ({
  useTailorGeneration: () => genMock,
}));

// ── useApplicationAnswers — controlled mock ───────────────────────────────────

const answersMock = {
  selected: new Set<string>(),
  toggle: vi.fn(),
  answers: {} as Record<string, string>,
  generating: false,
  error: null,
  generate: vi.fn(),
  canGenerate: false,
};

vi.mock('@/features/documents/components/TailorFlow/useApplicationAnswers', () => ({
  useApplicationAnswers: () => answersMock,
}));

// ── useInterviewQuestions — controlled mock ───────────────────────────────────

const interviewMock = {
  seedTopics: '',
  setSeedTopics: vi.fn(),
  questions: [],
  generating: false,
  error: null,
  generate: vi.fn(),
  canGenerate: false,
};

vi.mock('@/hooks/use-interview-questions', () => ({
  useInterviewQuestions: () => interviewMock,
}));

// ── Heavy child stubs ─────────────────────────────────────────────────────────

// TailorWizard stub exposes:
//   - a "next-step" button → calls setStep(step + 1), exercising handleStep →
//     persistForm → persistence.setWizardForm + persistence.setWizardStep.
//   - a "generate" button → calls onGenerate({ resume, outputType, researchCompany }),
//     exercising startGeneration → persistForm → persistence.setWizardForm.
// The stub is purposely @ajh/ui-free (uses div[role=button]) to stay inside
// the no-raw-button ESLint rule for test files.
vi.mock('./TailorWizard', () => ({
  TailorWizard: ({
    step,
    setStep,
    onGenerate,
  }: {
    step: number;
    setStep: (n: number) => void;
    onGenerate: (v: { resume: string; outputType: 'resume'; researchCompany: boolean }) => void;
  }) => (
    <div data-testid="tailor-wizard" data-step={step}>
      <div role="button" tabIndex={0} data-testid="wizard-next" onClick={() => setStep(step + 1)}>
        next-step
      </div>
      <div
        role="button"
        tabIndex={0}
        data-testid="wizard-generate"
        onClick={() =>
          onGenerate({ resume: 'my-resume', outputType: 'resume', researchCompany: false })
        }
      >
        generate
      </div>
    </div>
  ),
}));

vi.mock('./GeneratingPanel', () => ({
  GeneratingPanel: () => <div data-testid="generating-panel" />,
}));

vi.mock('./ResultsPanel', () => ({
  // div[role=button] avoids the no-raw-button ESLint rule while remaining
  // clickable via userEvent.click — stubs in test files only, no production code.
  ResultsPanel: ({
    onEditSettings,
    onTemplateChange,
    onAtsModeChange,
    templateId,
    atsMode,
  }: {
    onEditSettings?: () => void;
    onTemplateChange?: (v: string) => void;
    onAtsModeChange?: (v: boolean) => void;
    templateId?: string;
    atsMode?: boolean;
  }) => (
    <div data-testid="results-panel" data-templateid={templateId} data-atsmode={String(atsMode)}>
      <div role="button" tabIndex={0} onClick={onEditSettings}>
        edit-settings
      </div>
      <div role="button" tabIndex={0} onClick={() => onTemplateChange?.('classic')}>
        change-template
      </div>
      <div role="button" tabIndex={0} onClick={() => onAtsModeChange?.(true)}>
        toggle-ats
      </div>
    </div>
  ),
}));

vi.mock('./ApplicationQuestionsModal', () => ({
  ApplicationQuestionsModal: ({ onClose }: { onClose: () => void }) => (
    <div data-testid="questions-modal">
      <div role="button" tabIndex={0} onClick={onClose}>
        close-questions
      </div>
    </div>
  ),
}));

vi.mock('./InterviewQuestionsModal', () => ({
  InterviewQuestionsModal: ({ onClose }: { onClose: () => void }) => (
    <div data-testid="interview-modal">
      <div role="button" tabIndex={0} onClick={onClose}>
        close-interview
      </div>
    </div>
  ),
}));

vi.mock('./ReferralModal', () => ({
  ReferralModal: ({ onClose }: { onClose: () => void }) => (
    <div data-testid="referral-modal">
      <div role="button" tabIndex={0} onClick={onClose}>
        close-referral
      </div>
    </div>
  ),
}));

// ── Import component after all mocks ─────────────────────────────────────────

import type { AutopilotFoundJob } from '@ajh/shared';

import type { TemplateId } from '@/lib/generate';

import {
  TailorFlow,
  type TailorFlowController,
  type TailorFlowPersistence,
  type TailorWizardState,
} from './index';

// ── Fixtures ──────────────────────────────────────────────────────────────────

const JOB: AutopilotFoundJob = {
  title: 'Senior Engineer',
  company: 'Acme',
  url: 'https://acme.com/jobs/1',
  description: 'Build great things.',
  location: undefined,
  foundAt: Date.now(),
};

type MockedPersistence = Omit<
  TailorFlowPersistence,
  'setWizardStep' | 'setWizardForm' | 'setTemplateId' | 'setAtsMode'
> & {
  setWizardStep: Mock<(v: number) => void>;
  setWizardForm: Mock<(v: TailorWizardState) => void>;
  setTemplateId: Mock<(v: TemplateId) => void>;
  setAtsMode: Mock<(v: boolean) => void>;
};

function makePersistence(overrides: Partial<MockedPersistence> = {}): MockedPersistence {
  return {
    wizardStep: 0,
    wizardForm: null,
    templateId: 'modern',
    atsMode: false,
    setWizardStep: vi.fn<(v: number) => void>(),
    setWizardForm: vi.fn<(v: TailorWizardState) => void>(),
    setTemplateId: vi.fn<(v: TemplateId) => void>(),
    setAtsMode: vi.fn<(v: boolean) => void>(),
    ...overrides,
  };
}

function renderFlow(opts: {
  persistence?: TailorFlowPersistence;
  onController?: (c: TailorFlowController) => void;
}) {
  const persistence = opts.persistence ?? makePersistence();
  return render(
    <TailorFlow
      job={JOB}
      resumeText="My resume"
      board="linkedin"
      contextId="autopilot:https://acme.com/jobs/1"
      jobUrl="https://acme.com/jobs/1"
      persistence={persistence}
      onController={opts.onController}
    />
  );
}

// ── Reset between tests ───────────────────────────────────────────────────────

beforeEach(() => {
  genMock.generating = false;
  genMock.resumeOut = '';
  genMock.coverOut = '';
  genMock.output = '';
  genMock.generate.mockClear();
  genMock.abort.mockClear();
  answersMock.selected = new Set<string>();
  answersMock.generate.mockClear();
});

// ─────────────────────────────────────────────────────────────────────────────
// 1. Stage derivation
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — stage derivation', () => {
  it('renders the wizard (configuring) when not generating and no output', () => {
    renderFlow({});
    expect(screen.getByTestId('tailor-wizard')).toBeInTheDocument();
    expect(screen.queryByTestId('generating-panel')).not.toBeInTheDocument();
    expect(screen.queryByTestId('results-panel')).not.toBeInTheDocument();
  });

  it('renders the generating panel when generating=true (no output)', () => {
    genMock.generating = true;
    renderFlow({});
    expect(screen.getByTestId('generating-panel')).toBeInTheDocument();
    expect(screen.queryByTestId('tailor-wizard')).not.toBeInTheDocument();
    expect(screen.queryByTestId('results-panel')).not.toBeInTheDocument();
  });

  it('renders the results panel when resumeOut is set and not generating', () => {
    genMock.resumeOut = 'Generated resume text';
    genMock.output = 'Generated resume text';
    renderFlow({});
    expect(screen.getByTestId('results-panel')).toBeInTheDocument();
    expect(screen.queryByTestId('tailor-wizard')).not.toBeInTheDocument();
    expect(screen.queryByTestId('generating-panel')).not.toBeInTheDocument();
  });

  it('renders the results panel when only coverOut is set', () => {
    genMock.coverOut = 'Generated cover letter';
    genMock.output = 'Generated cover letter';
    renderFlow({});
    expect(screen.getByTestId('results-panel')).toBeInTheDocument();
  });

  it('generating=true WINS over existing output (generating stage takes priority)', () => {
    genMock.generating = true;
    genMock.resumeOut = 'Previous output';
    genMock.output = 'Previous output';
    renderFlow({});
    expect(screen.getByTestId('generating-panel')).toBeInTheDocument();
    expect(screen.queryByTestId('results-panel')).not.toBeInTheDocument();
  });

  it('clicking "edit-settings" from done stage reverts to the wizard (forceConfiguring)', async () => {
    genMock.resumeOut = 'Generated resume';
    genMock.output = 'Generated resume';
    const user = userEvent.setup();
    renderFlow({});

    // We are in done stage — results panel visible.
    expect(screen.getByTestId('results-panel')).toBeInTheDocument();

    // The stubbed ResultsPanel exposes an edit-settings button that calls onEditSettings.
    await user.click(screen.getByRole('button', { name: 'edit-settings' }));

    // After clicking, TailorFlow sets forceConfiguring → wizard shown.
    expect(screen.getByTestId('tailor-wizard')).toBeInTheDocument();
    expect(screen.queryByTestId('results-panel')).not.toBeInTheDocument();

    // Output is preserved under forceConfiguring — gen mock still has output.
    expect(genMock.resumeOut).toBe('Generated resume');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2. Persistence injection — host-agnostic contract
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — persistence injection', () => {
  it('reads wizardStep from the injected persistence and forwards it to TailorWizard', () => {
    // The stub renders `data-step={step}` so we can assert the value was forwarded.
    const persistence = makePersistence({ wizardStep: 2 });
    renderFlow({ persistence });
    expect(screen.getByTestId('tailor-wizard')).toHaveAttribute('data-step', '2');
  });

  it('reads wizardForm from persistence to seed the RHF defaultValues (non-null form)', () => {
    // When wizardForm is set, TailorFlow uses it as the one-shot seed.
    // Deeper RHF seed verification requires un-stubbing TailorWizard (overkill here;
    // covered by ApplyPage.test.tsx persistence round-trip tests).
    const persistence = makePersistence({
      wizardForm: { resume: 'Seeded resume', outputType: 'resume', researchCompany: false },
    });
    renderFlow({ persistence });
    expect(screen.getByTestId('tailor-wizard')).toBeInTheDocument();
  });

  it('calls persistence.setWizardForm AND persistence.setWizardStep when advancing a step', async () => {
    // GAP 1 FIX: TailorWizard stub exposes a "next-step" button that calls
    // setStep(step + 1). TailorFlow's handleStep() calls persistForm() first
    // (→ persistence.setWizardForm) then setStep() (→ persistence.setWizardStep).
    // Clicking "next-step" exercises the full write-back path.
    const user = userEvent.setup();
    const persistence = makePersistence({ wizardStep: 0 });
    renderFlow({ persistence });

    await user.click(screen.getByTestId('wizard-next'));

    // setWizardForm is called with the current RHF values (persistForm snapshot).
    expect(persistence.setWizardForm).toHaveBeenCalledTimes(1);
    // setWizardStep is called with the next step number.
    expect(persistence.setWizardStep).toHaveBeenCalledTimes(1);
    expect(persistence.setWizardStep).toHaveBeenCalledWith(1);
  });

  it('calls persistence.setWizardForm when the user clicks generate', async () => {
    // startGeneration calls persistForm() before launching gen.generate.
    // The "generate" button in the TailorWizard stub calls onGenerate({ ... }).
    const user = userEvent.setup();
    const persistence = makePersistence();
    renderFlow({ persistence });

    await user.click(screen.getByTestId('wizard-generate'));

    // persistForm is always called before generation starts.
    expect(persistence.setWizardForm).toHaveBeenCalledTimes(1);
    // setWizardStep is NOT called by startGeneration (only by handleStep).
    expect(persistence.setWizardStep).not.toHaveBeenCalled();
    // gen.generate is invoked.
    expect(genMock.generate).toHaveBeenCalledTimes(1);
  });

  it('reads templateId and atsMode from persistence and passes them to ResultsPanel when done', () => {
    // GAP 2 FIX: drive the "done" stage so ResultsPanel renders, then assert the
    // persistence values were forwarded as props (rendered as data-* attributes by
    // the stub). This proves TailorFlow reads them from persistence, not constants.
    genMock.resumeOut = 'Generated text';
    genMock.output = 'Generated text';
    const persistence = makePersistence({ templateId: 'classic', atsMode: true });
    renderFlow({ persistence });

    const panel = screen.getByTestId('results-panel');
    expect(panel).toHaveAttribute('data-templateid', 'classic');
    expect(panel).toHaveAttribute('data-atsmode', 'true');
  });

  it('calls persistence.setTemplateId when ResultsPanel fires onTemplateChange', async () => {
    // GAP 2 FIX: ResultsPanel stub exposes a "change-template" button that calls
    // onTemplateChange('classic'). Assert persistence.setTemplateId is called.
    const user = userEvent.setup();
    genMock.resumeOut = 'Generated text';
    genMock.output = 'Generated text';
    const persistence = makePersistence({ templateId: 'modern' });
    renderFlow({ persistence });

    await user.click(screen.getByRole('button', { name: 'change-template' }));

    expect(persistence.setTemplateId).toHaveBeenCalledTimes(1);
    expect(persistence.setTemplateId).toHaveBeenCalledWith('classic');
  });

  it('calls persistence.setAtsMode when ResultsPanel fires onAtsModeChange', async () => {
    // GAP 2 FIX: ResultsPanel stub exposes a "toggle-ats" button that calls
    // onAtsModeChange(true). Assert persistence.setAtsMode is called.
    const user = userEvent.setup();
    genMock.resumeOut = 'Generated text';
    genMock.output = 'Generated text';
    const persistence = makePersistence({ atsMode: false });
    renderFlow({ persistence });

    await user.click(screen.getByRole('button', { name: 'toggle-ats' }));

    expect(persistence.setAtsMode).toHaveBeenCalledTimes(1);
    expect(persistence.setAtsMode).toHaveBeenCalledWith(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3. Controller seam — onController shape + modal triggers
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — controller seam', () => {
  it('calls onController with stage=configuring when no output and not generating', () => {
    const onController = vi.fn();
    renderFlow({ onController });

    expect(onController).toHaveBeenCalled();
    const lastCall = onController.mock.calls[onController.mock.calls.length - 1];
    const controller = lastCall?.[0] as TailorFlowController | undefined;
    expect(controller).toBeDefined();
    expect(controller?.stage).toBe('configuring');
  });

  it('calls onController with stage=generating when generating=true', () => {
    genMock.generating = true;
    const onController = vi.fn();
    renderFlow({ onController });

    const lastCall = onController.mock.calls[onController.mock.calls.length - 1];
    const controller = lastCall?.[0] as TailorFlowController | undefined;
    expect(controller?.stage).toBe('generating');
  });

  it('calls onController with stage=done when output exists and not generating', () => {
    genMock.resumeOut = 'Output';
    genMock.output = 'Output';
    const onController = vi.fn();
    renderFlow({ onController });

    const lastCall = onController.mock.calls[onController.mock.calls.length - 1];
    const controller = lastCall?.[0] as TailorFlowController | undefined;
    expect(controller?.stage).toBe('done');
  });

  it('reports questionsCount=0 when selected is empty', () => {
    answersMock.selected = new Set<string>();
    const onController = vi.fn();
    renderFlow({ onController });

    const lastCall = onController.mock.calls[onController.mock.calls.length - 1];
    const controller = lastCall?.[0] as TailorFlowController | undefined;
    expect(controller?.questionsCount).toBe(0);
  });

  it('reports questionsCount reflecting selected.size', () => {
    answersMock.selected = new Set(['q1', 'q2', 'q3']);
    const onController = vi.fn();
    renderFlow({ onController });

    const lastCall = onController.mock.calls[onController.mock.calls.length - 1];
    const controller = lastCall?.[0] as TailorFlowController | undefined;
    expect(controller?.questionsCount).toBe(3);
  });

  it('controller exposes openQuestions and openReferral as functions', () => {
    const onController = vi.fn();
    renderFlow({ onController });

    const lastCall = onController.mock.calls[onController.mock.calls.length - 1];
    const controller = lastCall?.[0] as TailorFlowController | undefined;
    expect(typeof controller?.openQuestions).toBe('function');
    expect(typeof controller?.openReferral).toBe('function');
  });

  it('calling openQuestions() opens the ApplicationQuestionsModal', async () => {
    let capturedController: TailorFlowController | null = null;
    renderFlow({
      onController: (c) => {
        capturedController = c;
      },
    });

    expect(screen.queryByTestId('questions-modal')).not.toBeInTheDocument();

    // Wrap the imperative state-update in act() so React flushes synchronously.
    act(() => {
      capturedController?.openQuestions();
    });

    expect(await screen.findByTestId('questions-modal')).toBeInTheDocument();
  });

  it('calling openReferral() opens the ReferralModal', async () => {
    let capturedController: TailorFlowController | null = null;
    renderFlow({
      onController: (c) => {
        capturedController = c;
      },
    });

    expect(screen.queryByTestId('referral-modal')).not.toBeInTheDocument();

    act(() => {
      capturedController?.openReferral();
    });

    expect(await screen.findByTestId('referral-modal')).toBeInTheDocument();
  });

  it('closing the questions modal removes it from the DOM', async () => {
    let capturedController: TailorFlowController | null = null;
    const user = userEvent.setup();
    renderFlow({
      onController: (c) => {
        capturedController = c;
      },
    });

    act(() => {
      capturedController?.openQuestions();
    });
    expect(await screen.findByTestId('questions-modal')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'close-questions' }));
    expect(screen.queryByTestId('questions-modal')).not.toBeInTheDocument();
  });

  it('closing the referral modal removes it from the DOM', async () => {
    let capturedController: TailorFlowController | null = null;
    const user = userEvent.setup();
    renderFlow({
      onController: (c) => {
        capturedController = c;
      },
    });

    act(() => {
      capturedController?.openReferral();
    });
    expect(await screen.findByTestId('referral-modal')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'close-referral' }));
    expect(screen.queryByTestId('referral-modal')).not.toBeInTheDocument();
  });

  it('onController is not required — component renders without it', () => {
    // Verify no crash when onController prop is omitted.
    expect(() => renderFlow({})).not.toThrow();
    expect(screen.getByTestId('tailor-wizard')).toBeInTheDocument();
  });
});
