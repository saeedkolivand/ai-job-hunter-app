/**
 * Shared mocks, state and render helpers for the TailorFlow tests

 *
 * The `vi.mock` calls live HERE (hoisted above this module's imports), so importing
 * this module first registers them for the component under test — no per-file copies.
 *
 * Strategy:
 *  - `useTailorPipeline` and `useApplicationAnswers` are mocked so stage
 *    transitions are fully controlled without any IPC / React Query.
 *  - Service hooks (`useExtractText`, `useResolveJobUrl`, `useSelectedModel`,
 *    `useCanUseAI`) are mocked so no QueryClient / AppClient provider is needed.
 *  - Heavy child panels (TailorWizard, GeneratingPanel, ResultsPanel,
 *    ApplicationQuestionsModal, ReferralModal) are stubbed to stable markers so
 *    assertions are cheap and deterministic.
 *  - `motion/react` is collapsed to plain fragments (no animation overhead).
 *  - `@ajh/translations` returns keys as-is.
 */
import React from 'react';
import { type Mock, vi } from 'vitest';
import { render } from '@testing-library/react';

import type { AutopilotFoundJob } from '@ajh/shared';

import { TailorFlow, type TailorFlowController, type TailorFlowPersistence } from './index';

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
  useSelectedProvider: () => 'ollama',
}));

// ── Service hooks — no real IPC / QueryClient needed ─────────────────────────

// Mutable container so individual tests can override the resolved description
// and the second arg (shouldFetch) can be captured and asserted.
export const resolveJobUrlState = {
  data: undefined as { description: string } | undefined,
  isLoading: false,
  /** The last `shouldFetch` arg `useResolveJobUrl` received. */
  lastShouldFetch: undefined as boolean | undefined,
};

// Mutable container so tests can flip the active model's web-search capability,
// which drives the capability-driven default of the "search company" toggle.
export const modelCapsState = {
  data: { supportsWebSearch: false } as { supportsWebSearch: boolean } | undefined,
  isSuccess: true,
};

/** Saved documents `useDefaultResumeId` resolves the Score-tab fallback from. */
export const docsState: { docs: { _id: string; name?: string; isDefault?: boolean }[] } = {
  docs: [],
};

vi.mock('@/services', () => ({
  // `TailorFlow` resolves the DEFAULT résumé for the Score tab's fallback id
  // (`useDefaultResumeId` reads this). Defaults to an empty list — no default
  // résumé — which keeps every other test on the pre-existing behaviour.
  useDocuments: () => ({ data: docsState.docs, isLoading: false }),
  useResolveJobUrl: (_url: string, shouldFetch: boolean) => {
    resolveJobUrlState.lastShouldFetch = shouldFetch;
    return { data: resolveJobUrlState.data, isLoading: resolveJobUrlState.isLoading };
  },
  useExtractText: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useActiveModelCapabilities: () => ({
    data: modelCapsState.data,
    isSuccess: modelCapsState.isSuccess,
  }),
}));

// ── useTailorPipeline — controlled mock ───────────────────────────────────────

export const genMock = {
  state: 'idle' as string,
  busy: false,
  starting: false,
  currentStep: 0,
  stageLabel: '',
  thinking: '',
  draft: '',
  letterDraft: '',
  resumeOut: '' as string,
  coverOut: '' as string,
  activeOut: 'resume' as const,
  setActiveOut: vi.fn() as Mock,
  output: '' as string,
  hasOutput: false,
  error: null as string | null,
  stoppedReason: undefined as string | null | undefined,
  copied: false,
  exportOpen: false,
  setExportOpen: vi.fn() as Mock,
  start: vi.fn().mockResolvedValue(null) as Mock,
  cancel: vi.fn() as Mock,
  copy: vi.fn() as Mock,
  exportAs: vi.fn() as Mock,
  editActiveOutput: vi.fn() as Mock,
  meta: null,
  market: undefined as string | undefined,
  report: null,
  pipelineReview: undefined,
  recheck: undefined,
  rechecking: false,
  runs: [],
};

vi.mock('@/features/documents/components/TailorFlow/useTailorPipeline', () => ({
  useTailorPipeline: () => genMock,
}));

// ── useApplicationAnswers — controlled mock ───────────────────────────────────

export const answersMock = {
  selected: new Set<string>(),
  toggle: vi.fn() as Mock,
  answers: {} as Record<string, string>,
  generating: false,
  error: null,
  generate: vi.fn() as Mock,
  canGenerate: false,
};

vi.mock('@/features/documents/components/TailorFlow/useApplicationAnswers', () => ({
  useApplicationAnswers: () => answersMock,
}));

// ── useInterviewQuestions — controlled mock ───────────────────────────────────

const interviewMock = {
  seedTopics: '',
  setSeedTopics: vi.fn() as Mock,
  audiences: ['recruiter', 'hiringManager'],
  toggleAudience: vi.fn() as Mock,
  questions: [],
  generating: false,
  error: null,
  generate: vi.fn() as Mock,
  canGenerate: false,
  needsResearchKey: false,
};

vi.mock('@/hooks/use-interview-questions', () => ({
  useInterviewQuestions: () => interviewMock,
}));

// ── useJobAdSummary — controlled mock ─────────────────────────────────────────

// Hoisted so the spies are STABLE across hook calls/renders — recreating them
// per call would make any assertion against them brittle (cleared in beforeEach).
export const jobAdSummaryMock = { generate: vi.fn() as Mock, setLanguage: vi.fn() as Mock };

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

// ── Heavy child stubs (the components live in `TailorFlow.test-stubs.tsx`) ────

vi.mock('./TailorWizard', async () => (await import('./TailorFlow.test-stubs')).wizardModule);
vi.mock(
  './GeneratingPanel',
  async () => (await import('./TailorFlow.test-stubs')).generatingPanelModule
);
vi.mock('./ResultsPanel', async () => (await import('./TailorFlow.test-stubs')).resultsPanelModule);
vi.mock(
  './ApplicationQuestionsModal',
  async () => (await import('./TailorFlow.test-stubs')).questionsModalModule
);
vi.mock(
  './InterviewQuestionsModal',
  async () => (await import('./TailorFlow.test-stubs')).interviewModalModule
);
vi.mock(
  './ReferralModal',
  async () => (await import('./TailorFlow.test-stubs')).referralModalModule
);

// ── Fixtures ──────────────────────────────────────────────────────────────────

export const JOB: AutopilotFoundJob = {
  title: 'Senior Engineer',
  company: 'Acme',
  url: 'https://acme.com/jobs/1',
  description: 'Build great things.',
  location: undefined,
  foundAt: Date.now(),
};

type MockedPersistence = Omit<
  TailorFlowPersistence,
  | 'setWizardStep'
  | 'setWizardForm'
  | 'setTemplateId'
  | 'setAtsMode'
  | 'setAccent'
  | 'setLetterLayoutId'
  | 'setRun'
> & {
  setWizardStep: Mock;
  setWizardForm: Mock;
  setTemplateId: Mock;
  setAtsMode: Mock;
  setAccent: Mock;
  setLetterLayoutId: Mock;
  setRun: Mock;
};

export function makePersistence(overrides: Partial<MockedPersistence> = {}): MockedPersistence {
  return {
    wizardStep: 0,
    wizardForm: null,
    templateId: 'classic',
    atsMode: false,
    runId: null,
    runJobId: null,
    setWizardStep: vi.fn(),
    setWizardForm: vi.fn(),
    setTemplateId: vi.fn(),
    setAtsMode: vi.fn(),
    setAccent: vi.fn(),
    setLetterLayoutId: vi.fn(),
    setRun: vi.fn(),
    ...overrides,
  };
}

export function renderFlow(opts: {
  persistence?: TailorFlowPersistence;
  onController?: (c: TailorFlowController) => void;
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
      persistence={persistence}
      onController={opts.onController}
      onJobDescChange={opts.onJobDescChange}
    />
  );
}

/** A fresh element per render so React reconciles (identical element refs bail). */
export const rerenderFlow = (persistence: TailorFlowPersistence = makePersistence()) => (
  <TailorFlow
    job={JOB}
    resumeText="My resume"
    board="linkedin"
    contextId="autopilot:https://acme.com/jobs/1"
    jobUrl="https://acme.com/jobs/1"
    persistence={persistence}
  />
);

/** Call in `beforeEach`. */
export function resetState() {
  docsState.docs = [];
  genMock.state = 'idle';
  genMock.busy = false;
  genMock.hasOutput = false;
  genMock.resumeOut = '';
  genMock.coverOut = '';
  genMock.output = '';
  genMock.error = null;
  genMock.market = undefined;
  genMock.start.mockClear();
  genMock.cancel.mockClear();
  jobAdSummaryMock.generate.mockClear();
  jobAdSummaryMock.setLanguage.mockClear();
  answersMock.selected = new Set<string>();
  answersMock.generate.mockClear();
  // Reset useResolveJobUrl state.
  resolveJobUrlState.data = undefined;
  resolveJobUrlState.isLoading = false;
  resolveJobUrlState.lastShouldFetch = undefined;
  // Reset model-capability state (default: cannot web-search → toggle off).
  modelCapsState.data = { supportsWebSearch: false };
  modelCapsState.isSuccess = true;
}
