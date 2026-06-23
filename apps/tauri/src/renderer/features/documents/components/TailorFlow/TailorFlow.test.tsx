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

import { TEST_IDS } from '@ajh/test-ids';

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

// Mutable container so individual tests can override the resolved description
// and the second arg (shouldFetch) can be captured and asserted.
const resolveJobUrlState = {
  data: undefined as { description: string } | undefined,
  isLoading: false,
};
// Tracks the last `shouldFetch` arg received by useResolveJobUrl.
let lastResolveJobUrlShouldFetch: boolean | undefined = undefined;

vi.mock('@/services', () => ({
  useResolveJobUrl: (_url: string, shouldFetch: boolean) => {
    lastResolveJobUrlShouldFetch = shouldFetch;
    return { data: resolveJobUrlState.data, isLoading: resolveJobUrlState.isLoading };
  },
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
  hydrate: vi.fn(),
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
  audiences: ['recruiter', 'hiringManager'],
  toggleAudience: vi.fn(),
  questions: [],
  generating: false,
  error: null,
  generate: vi.fn(),
  canGenerate: false,
  needsResearchKey: false,
};

vi.mock('@/hooks/use-interview-questions', () => ({
  useInterviewQuestions: () => interviewMock,
}));

// ── useJobAdSummary — controlled mock ─────────────────────────────────────────

// Hoisted so the spies are STABLE across hook calls/renders — recreating them
// per call would make any assertion against them brittle (cleared in beforeEach).
const jobAdSummaryMock = vi.hoisted(() => ({
  generate: vi.fn(),
  setLanguage: vi.fn(),
}));

vi.mock('./useJobAdSummary', () => ({
  useJobAdSummary: () => ({
    summary: '',
    generating: false,
    error: null,
    generate: jobAdSummaryMock.generate,
    language: 'en',
    setLanguage: jobAdSummaryMock.setLanguage,
  }),
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
    jobDesc,
    onJobDescChange,
  }: {
    step: number;
    setStep: (n: number) => void;
    onGenerate: (v: { resume: string; outputType: 'resume'; researchCompany: boolean }) => void;
    jobDesc?: string;
    onJobDescChange?: (v: string) => void;
  }) => (
    <div data-testid={TEST_IDS.documents.tailorWizard} data-step={step} data-jobdesc={jobDesc}>
      <div
        role="button"
        tabIndex={0}
        data-testid={TEST_IDS.documents.wizardNext}
        onClick={() => setStep(step + 1)}
      >
        next-step
      </div>
      <div
        role="button"
        tabIndex={0}
        data-testid={TEST_IDS.documents.wizardGenerate}
        onClick={() =>
          onGenerate({ resume: 'my-resume', outputType: 'resume', researchCompany: false })
        }
      >
        generate
      </div>
      <div
        role="button"
        tabIndex={0}
        data-testid="wizard-edit-jobdesc"
        onClick={() => onJobDescChange?.('edited-job-ad')}
      >
        edit-jobdesc
      </div>
    </div>
  ),
}));

vi.mock('./GeneratingPanel', () => ({
  GeneratingPanel: () => <div data-testid={TEST_IDS.documents.generatingPanel} />,
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
    <div
      data-testid={TEST_IDS.documents.resultsPanel}
      data-templateid={templateId}
      data-atsmode={String(atsMode)}
    >
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
    <div data-testid={TEST_IDS.documents.questionsModal}>
      <div role="button" tabIndex={0} onClick={onClose}>
        close-questions
      </div>
    </div>
  ),
}));

vi.mock('./InterviewQuestionsModal', () => ({
  InterviewQuestionsModal: ({ onClose }: { onClose: () => void }) => (
    <div data-testid={TEST_IDS.documents.interviewModal}>
      <div role="button" tabIndex={0} onClick={onClose}>
        close-interview
      </div>
    </div>
  ),
}));

vi.mock('./ReferralModal', () => ({
  ReferralModal: ({ onClose }: { onClose: () => void }) => (
    <div data-testid={TEST_IDS.documents.referralModal}>
      <div role="button" tabIndex={0} onClick={onClose}>
        close-referral
      </div>
    </div>
  ),
}));

// ── Import component after all mocks ─────────────────────────────────────────

import type { AiGenerationRecord, AutopilotFoundJob } from '@ajh/shared';

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

const SAVED_GENERATION: AiGenerationRecord = {
  id: 'rec-1',
  createdAt: Date.now(),
  candidateName: 'Ada Lovelace',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  targetLanguage: 'en',
  mismatch: false,
  topRequirements: ['rust'],
  mode: 'ats',
  resumeText: 'SAVED RESUME',
  coverLetterText: 'SAVED COVER',
  jobAd: 'Build great things.',
  jobUrl: 'https://acme.com/jobs/1',
  board: 'linkedin',
  applicationAnswers: [],
  companyBrief: '',
  interviewQuestions: [],
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
  seedGeneration?: AiGenerationRecord;
  job?: AutopilotFoundJob;
  onJobDescChange?: (text: string) => void;
}) {
  const persistence = opts.persistence ?? makePersistence();
  const job = opts.job ?? JOB;
  return render(
    <TailorFlow
      job={job}
      resumeText="My resume"
      board="linkedin"
      contextId="autopilot:https://acme.com/jobs/1"
      jobUrl="https://acme.com/jobs/1"
      seedGeneration={opts.seedGeneration}
      persistence={persistence}
      onController={opts.onController}
      onJobDescChange={opts.onJobDescChange}
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
  genMock.hydrate.mockClear();
  jobAdSummaryMock.generate.mockClear();
  jobAdSummaryMock.setLanguage.mockClear();
  answersMock.selected = new Set<string>();
  answersMock.generate.mockClear();
  // Reset useResolveJobUrl state.
  resolveJobUrlState.data = undefined;
  resolveJobUrlState.isLoading = false;
  lastResolveJobUrlShouldFetch = undefined;
});

// ─────────────────────────────────────────────────────────────────────────────
// 1. Stage derivation
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — stage derivation', () => {
  it('renders the wizard (configuring) when not generating and no output', () => {
    renderFlow({});
    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.generatingPanel)).not.toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.resultsPanel)).not.toBeInTheDocument();
  });

  it('renders the generating panel when generating=true (no output)', () => {
    genMock.generating = true;
    renderFlow({});
    expect(screen.getByTestId(TEST_IDS.documents.generatingPanel)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.tailorWizard)).not.toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.resultsPanel)).not.toBeInTheDocument();
  });

  it('renders the results panel when resumeOut is set and not generating', () => {
    genMock.resumeOut = 'Generated resume text';
    genMock.output = 'Generated resume text';
    renderFlow({});
    expect(screen.getByTestId(TEST_IDS.documents.resultsPanel)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.tailorWizard)).not.toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.generatingPanel)).not.toBeInTheDocument();
  });

  it('renders the results panel when only coverOut is set', () => {
    genMock.coverOut = 'Generated cover letter';
    genMock.output = 'Generated cover letter';
    renderFlow({});
    expect(screen.getByTestId(TEST_IDS.documents.resultsPanel)).toBeInTheDocument();
  });

  it('generating=true WINS over existing output (generating stage takes priority)', () => {
    genMock.generating = true;
    genMock.resumeOut = 'Previous output';
    genMock.output = 'Previous output';
    renderFlow({});
    expect(screen.getByTestId(TEST_IDS.documents.generatingPanel)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.resultsPanel)).not.toBeInTheDocument();
  });

  it('clicking "edit-settings" from done stage reverts to the wizard (forceConfiguring)', async () => {
    genMock.resumeOut = 'Generated resume';
    genMock.output = 'Generated resume';
    const user = userEvent.setup();
    renderFlow({});

    // We are in done stage — results panel visible.
    expect(screen.getByTestId(TEST_IDS.documents.resultsPanel)).toBeInTheDocument();

    // The stubbed ResultsPanel exposes an edit-settings button that calls onEditSettings.
    await user.click(screen.getByRole('button', { name: 'edit-settings' }));

    // After clicking, TailorFlow sets forceConfiguring → wizard shown.
    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.resultsPanel)).not.toBeInTheDocument();

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
    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toHaveAttribute('data-step', '2');
  });

  it('reads wizardForm from persistence to seed the RHF defaultValues (non-null form)', () => {
    // When wizardForm is set, TailorFlow uses it as the one-shot seed.
    // Deeper RHF seed verification requires un-stubbing TailorWizard (overkill here;
    // covered by ApplyPage.test.tsx persistence round-trip tests).
    const persistence = makePersistence({
      wizardForm: { resume: 'Seeded resume', outputType: 'resume', researchCompany: false },
    });
    renderFlow({ persistence });
    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toBeInTheDocument();
  });

  it('calls persistence.setWizardForm AND persistence.setWizardStep when advancing a step', async () => {
    // GAP 1 FIX: TailorWizard stub exposes a "next-step" button that calls
    // setStep(step + 1). TailorFlow's handleStep() calls persistForm() first
    // (→ persistence.setWizardForm) then setStep() (→ persistence.setWizardStep).
    // Clicking "next-step" exercises the full write-back path.
    const user = userEvent.setup();
    const persistence = makePersistence({ wizardStep: 0 });
    renderFlow({ persistence });

    await user.click(screen.getByTestId(TEST_IDS.documents.wizardNext));

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

    await user.click(screen.getByTestId(TEST_IDS.documents.wizardGenerate));

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

    const panel = screen.getByTestId(TEST_IDS.documents.resultsPanel);
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

    expect(screen.queryByTestId(TEST_IDS.documents.questionsModal)).not.toBeInTheDocument();

    // Wrap the imperative state-update in act() so React flushes synchronously.
    act(() => {
      capturedController?.openQuestions();
    });

    expect(await screen.findByTestId(TEST_IDS.documents.questionsModal)).toBeInTheDocument();
  });

  it('calling openReferral() opens the ReferralModal', async () => {
    let capturedController: TailorFlowController | null = null;
    renderFlow({
      onController: (c) => {
        capturedController = c;
      },
    });

    expect(screen.queryByTestId(TEST_IDS.documents.referralModal)).not.toBeInTheDocument();

    act(() => {
      capturedController?.openReferral();
    });

    expect(await screen.findByTestId(TEST_IDS.documents.referralModal)).toBeInTheDocument();
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
    expect(await screen.findByTestId(TEST_IDS.documents.questionsModal)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'close-questions' }));
    expect(screen.queryByTestId(TEST_IDS.documents.questionsModal)).not.toBeInTheDocument();
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
    expect(await screen.findByTestId(TEST_IDS.documents.referralModal)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'close-referral' }));
    expect(screen.queryByTestId(TEST_IDS.documents.referralModal)).not.toBeInTheDocument();
  });

  it('onController is not required — component renders without it', () => {
    // Verify no crash when onController prop is omitted.
    expect(() => renderFlow({})).not.toThrow();
    expect(screen.getByTestId(TEST_IDS.documents.tailorWizard)).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4. Cold-entry hydration — seedGeneration → gen.hydrate
// ─────────────────────────────────────────────────────────────────────────────

describe('TailorFlow — cold-entry hydration', () => {
  it('hydrates the session from seedGeneration (mapped text + id + meta)', () => {
    renderFlow({ seedGeneration: SAVED_GENERATION });

    expect(genMock.hydrate).toHaveBeenCalledTimes(1);
    expect(genMock.hydrate).toHaveBeenCalledWith({
      resumeOut: 'SAVED RESUME',
      coverOut: 'SAVED COVER',
      savedId: 'rec-1',
      meta: {
        candidateName: 'Ada Lovelace',
        jobTitle: 'Senior Engineer',
        companyName: 'Acme',
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        targetLanguage: 'en',
        topRequirements: ['rust'],
      },
    });
  });

  it('does not hydrate when no seedGeneration is provided', () => {
    renderFlow({});
    expect(genMock.hydrate).not.toHaveBeenCalled();
  });

  it('does not hydrate when the saved record has no résumé or cover text', () => {
    renderFlow({
      seedGeneration: { ...SAVED_GENERATION, resumeText: '', coverLetterText: '' },
    });
    expect(genMock.hydrate).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5. prefer-longer / skip-refetch branch (SHORT_DESC_FLOOR = 800)
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
    expect(lastResolveJobUrlShouldFetch).toBe(false);
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
// 6. onJobDescChange prop — host persist callback
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
