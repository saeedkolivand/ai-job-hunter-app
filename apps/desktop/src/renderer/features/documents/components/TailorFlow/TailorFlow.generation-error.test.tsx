/**
 * TailorFlow — generation-failure visibility.
 *
 * A failed run used to fall back to the wizard SILENTLY: `runTailor` wrote
 * `session.error`, but `TailorFlow` never read it and no toast fired. These
 * tests drive a failure through the REAL `useTailorGeneration` hook + the
 * real `useGenerationStore` (only the AI pipeline functions are stubbed), and
 * assert the rendered DOM — a test that only checked `gen.error` would have
 * passed against the broken code, since the store already set that field.
 *
 * Everything NOT under test (heavy result/wizard panels, the sibling
 * assistants, service hooks) is stubbed exactly as in `TailorFlow.test.tsx`.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { AutopilotFoundJob } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

import { generateResume } from '@/lib/generate';
import { useGenerationStore } from '@/store/generation-store';
import { makeQueryClient } from '@/test-support';

// ── i18n — identity translator so the raw keys are assertable ────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── @ajh/ui — keep everything real (incl. ErrorState) except useNotification ─

const mockNotify = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return { ...actual, useNotification: () => mockNotify };
});

// ── motion/react — collapse animations ────────────────────────────────────────

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

// ── ModelSelector + service hooks — static, no IPC ────────────────────────────

vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
  useCanUseAI: () => ({ canUse: true, reason: undefined }),
}));

vi.mock('@/services', () => ({
  useResolveJobUrl: () => ({ data: undefined, isLoading: false }),
  useExtractText: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useActiveModelCapabilities: () => ({ data: { supportsWebSearch: false }, isSuccess: true }),
}));

// ── AppClient — only `aiGenerations.save` is exercised on a clean run ────────

const save = vi.fn().mockResolvedValue({ id: 'gen-1', success: true });
vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({ aiGenerations: { save } }),
}));

// ── The AI pipeline — the only seam driving success vs. failure ──────────────

vi.mock('@/lib/generate', () => ({
  extractMetadata: vi.fn().mockResolvedValue({
    candidateName: '',
    jobTitle: '',
    companyName: '',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
    topRequirements: [],
  }),
  generateResume: vi.fn().mockResolvedValue('RESUME TEXT'),
  generateCoverLetter: vi.fn().mockResolvedValue({ text: 'COVER', companyBrief: '' }),
  buildFilename: vi.fn(),
  exportDOCX: vi.fn(),
  exportPDF: vi.fn(),
  exportTXT: vi.fn(),
  computeQualityReport: vi.fn().mockResolvedValue(null),
  serializeQualityReport: vi.fn(),
  parseQualityReport: vi.fn().mockReturnValue(null),
}));

// ── Sibling assistants — irrelevant to this failure path ─────────────────────

vi.mock('@/features/documents/components/TailorFlow/useApplicationAnswers', () => ({
  useApplicationAnswers: () => ({
    selected: new Set<string>(),
    toggle: vi.fn(),
    answers: {},
    generating: false,
    error: null,
    generate: vi.fn(),
    canGenerate: false,
  }),
}));

vi.mock('@/hooks/use-interview-questions', () => ({
  useInterviewQuestions: () => ({
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
  }),
}));

vi.mock('./useJobAdSummary', () => ({
  useJobAdSummary: () => ({
    summary: '',
    generating: false,
    error: null,
    generate: vi.fn(),
    language: 'en',
    setLanguage: vi.fn(),
  }),
}));

// ── Heavy child stubs — TailorWizard exposes a "generate" button ─────────────

vi.mock('./TailorWizard', () => ({
  TailorWizard: ({
    onGenerate,
    methods,
  }: {
    onGenerate: (v: { resume: string; outputType: 'resume'; researchCompany: boolean }) => void;
    methods: { watch: (name: 'researchCompany') => boolean };
  }) => (
    <div
      data-testid={TEST_IDS.documents.tailorWizard}
      data-research={String(methods.watch('researchCompany'))}
    >
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
    </div>
  ),
}));

vi.mock('./GeneratingPanel', () => ({
  GeneratingPanel: () => <div data-testid={TEST_IDS.documents.generatingPanel} />,
}));

vi.mock('./ResultsPanel', () => ({
  ResultsPanel: () => <div data-testid={TEST_IDS.documents.resultsPanel} />,
}));

vi.mock('./ApplicationQuestionsModal', () => ({ ApplicationQuestionsModal: () => null }));
vi.mock('./InterviewQuestionsModal', () => ({ InterviewQuestionsModal: () => null }));
vi.mock('./ReferralModal', () => ({ ReferralModal: () => null }));

// ── Import after all mocks ────────────────────────────────────────────────────

import { TailorFlow } from './index';

const JOB: AutopilotFoundJob = {
  title: 'Senior Engineer',
  company: 'Acme',
  url: 'https://acme.com/jobs/1',
  description: 'Build great things.',
  location: undefined,
  foundAt: Date.now(),
};

function makePersistence() {
  return {
    wizardStep: 0,
    wizardForm: null,
    templateId: 'classic' as const,
    atsMode: false,
    setWizardStep: vi.fn(),
    setWizardForm: vi.fn(),
    setTemplateId: vi.fn(),
    setAtsMode: vi.fn(),
    setAccent: vi.fn(),
    setLetterLayoutId: vi.fn(),
  };
}

function renderFlow() {
  const client = makeQueryClient();
  return render(
    <QueryClientProvider client={client}>
      <TailorFlow
        job={JOB}
        resumeText="My resume"
        board="linkedin"
        contextId="autopilot:https://acme.com/jobs/1"
        jobUrl="https://acme.com/jobs/1"
        persistence={makePersistence()}
      />
    </QueryClientProvider>
  );
}

beforeEach(() => {
  // The generation session lives in a module-level store — reset between tests.
  useGenerationStore.setState({ sessions: {} });
  save.mockClear();
  mockNotify.error.mockClear();
  vi.mocked(generateResume).mockClear();
  vi.mocked(generateResume).mockResolvedValue('RESUME TEXT');
});

describe('TailorFlow — a failed generation surfaces its reason', () => {
  it('renders the failure message on the wizard and fires a toast', async () => {
    vi.mocked(generateResume).mockRejectedValueOnce(new Error('Model timed out'));
    const user = userEvent.setup();
    renderFlow();

    await user.click(screen.getByTestId(TEST_IDS.documents.wizardGenerate));

    const banner = await screen.findByTestId(TEST_IDS.documents.generationError);
    expect(banner).toHaveTextContent('Model timed out');
    expect(mockNotify.error).toHaveBeenCalledWith({ message: 'autopilot.apply.failed' });
  });

  it('clears a stale error once a subsequent run succeeds', async () => {
    vi.mocked(generateResume).mockRejectedValueOnce(new Error('boom'));
    const user = userEvent.setup();
    renderFlow();

    await user.click(screen.getByTestId(TEST_IDS.documents.wizardGenerate));
    expect(await screen.findByTestId(TEST_IDS.documents.generationError)).toBeInTheDocument();

    // The next attempt succeeds (base mock resolves) — the stale failure must
    // not sit on screen over the fresh, in-flight/successful run.
    await user.click(screen.getByTestId(TEST_IDS.documents.wizardGenerate));

    expect(await screen.findByTestId(TEST_IDS.documents.resultsPanel)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.generationError)).not.toBeInTheDocument();
  });
});
