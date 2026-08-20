/**
 * TailorFlow — staged-pipeline start-failure visibility.
 *
 * A failed run used to fall back to the wizard SILENTLY on the fast path;
 * these tests drive a failure through the REAL `useTailorPipeline` →
 * `useResumePipelineSession` → service-hook chain against a mock `AppClient`
 * (only `resumePipeline.run`/`.get` are stubbed) and assert the rendered
 * DOM — a test that only checked a mocked hook's `error` field would pass
 * against broken wiring, since a shallow mock already sets that field.
 *
 * Everything NOT under test (heavy result/wizard panels, the sibling
 * assistants, service hooks unrelated to the pipeline) is stubbed exactly as
 * in `TailorFlow.test.tsx`.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { AutopilotFoundJob } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';
import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

import { createMockClient, withProviders } from '@/test-support';

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

// ── ModelSelector + service hooks — static, no IPC for these ─────────────────

vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
  useCanUseAI: () => ({ canUse: true, reason: undefined }),
  useSelectedProvider: () => 'ollama',
}));

vi.mock('@/services', () => ({
  // `TailorFlow` resolves the DEFAULT résumé for the Score tab's fallback id
  // (`useDefaultResumeId` reads this) — empty list = no default, which keeps
  // these tests on the pre-existing 'no résumé' behaviour.
  useDocuments: () => ({ data: [], isLoading: false }),
  useResolveJobUrl: () => ({ data: undefined, isLoading: false }),
  useExtractText: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useActiveModelCapabilities: () => ({ data: { supportsWebSearch: false }, isSuccess: true }),
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
    runId: null,
    runJobId: null,
    setWizardStep: vi.fn(),
    setWizardForm: vi.fn(),
    setTemplateId: vi.fn(),
    setAtsMode: vi.fn(),
    setAccent: vi.fn(),
    setLetterLayoutId: vi.fn(),
    setRun: vi.fn(),
  };
}

function completedDetail(overrides: Partial<PipelineRunDetail> = {}): PipelineRunDetail {
  return {
    runId: 'run-1',
    jobUrl: JOB.url,
    kind: 'resume',
    depth: 'quality',
    status: 'completed',
    startedAt: 1,
    metrics: {},
    events: [],
    report: null,
    resumeText: 'RESUME TEXT',
    ...overrides,
  };
}

function renderFlow(overrides: Record<string, (...args: never[]) => unknown> = {}) {
  const client = createMockClient(overrides);
  return render(
    <TailorFlow
      job={JOB}
      resumeText="My resume"
      board="linkedin"
      contextId="autopilot:https://acme.com/jobs/1"
      jobUrl="https://acme.com/jobs/1"
      persistence={makePersistence()}
    />,
    { wrapper: withProviders(client) }
  );
}

beforeEach(() => {
  mockNotify.error.mockClear();
});

describe('TailorFlow — a failed staged-run start surfaces its reason', () => {
  it('renders the failure message on the wizard and fires a toast', async () => {
    const user = userEvent.setup();
    renderFlow({
      'resumePipeline.run': vi.fn().mockRejectedValueOnce(new Error('Model timed out')),
    });

    await user.click(screen.getByTestId(TEST_IDS.documents.wizardGenerate));

    const banner = await screen.findByTestId(TEST_IDS.documents.generationError);
    expect(banner).toHaveTextContent('Model timed out');
    expect(mockNotify.error).toHaveBeenCalledWith({ message: 'autopilot.apply.failed' });
  });

  it('clears a stale error once a subsequent run completes', async () => {
    const run = vi
      .fn()
      .mockRejectedValueOnce(new Error('boom'))
      .mockResolvedValueOnce({ runId: 'run-1', jobId: 'job-1' });
    const user = userEvent.setup();
    renderFlow({
      'resumePipeline.run': run,
      'resumePipeline.get': vi.fn().mockResolvedValue(completedDetail()),
    });

    await user.click(screen.getByTestId(TEST_IDS.documents.wizardGenerate));
    expect(await screen.findByTestId(TEST_IDS.documents.generationError)).toBeInTheDocument();

    // The next attempt succeeds and the run completes immediately — the stale
    // failure must not sit on screen over the fresh, successful run.
    await user.click(screen.getByTestId(TEST_IDS.documents.wizardGenerate));

    expect(await screen.findByTestId(TEST_IDS.documents.resultsPanel)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.generationError)).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// F1 — mount-level regression through a DocumentsTab-shaped host
// ─────────────────────────────────────────────────────────────────────────────
// `TailorFlow.test.tsx` mocks `useTailorPipeline` wholesale and passes a
// STABLE `vi.fn()` persistence, so it never exercised the real loop: this
// file's REAL `useTailorPipeline` chain (already used above) is reused here
// with a host that mirrors the actual production shape — `runId`/`runJobId`
// in a single OBJECT slice, always replaced with a fresh `{...prev, ...patch}`
// spread (mirrors Zustand's `setApplicationApply` exactly — a plain primitive
// `useState` would let React bail out on an equal value, silently masking the
// defect a real store's always-new-object setter never would), and a
// brand-new `persistence` object literal (and `setRun` arrow) on every
// render, exactly like `DocumentsTab`.
//
// Disproven by mutation-testing this file against the reverted (buggy)
// source: it does NOT reproduce "Maximum update depth exceeded" in this
// jsdom/RTL environment — the effect-driven render→setState cycle the bug
// describes doesn't hit React's nested-update-limit within one `act()`
// flush here, unlike a real browser tab. The call-count assertion below
// DOES catch the regression (verified red against the reverted source): the
// buggy effect calls `setRun` repeatedly (4 times observed here — bounded
// in THIS test only because a mock AppClient settles quickly; production's
// unbounded churn is what the bug report's repro actually crashed on) for
// the SAME run instead of once. The hook-level test in
// `useTailorPipeline.test.ts` remains the primary, crash-shaped guard.

function DocumentsTabShapedHost({ onSetRun }: { onSetRun: (ids: unknown) => void }) {
  const [applyRun, setApplyRun] = React.useState<{ runId: string; jobId: string } | null>(null);
  // A fresh object every render — never memoized, matching DocumentsTab's
  // inline `const persistence: TailorFlowPersistence = { ... }`.
  const persistence = {
    ...makePersistence(),
    runId: applyRun?.runId ?? null,
    runJobId: applyRun?.jobId ?? null,
    setRun: (ids: { runId: string; jobId: string } | null) => {
      onSetRun(ids);
      setApplyRun((prev) => (ids ? { ...prev, ...ids } : null));
    },
  };
  return (
    <TailorFlow
      job={JOB}
      resumeText="My resume"
      board="linkedin"
      contextId="autopilot:https://acme.com/jobs/1"
      jobUrl="https://acme.com/jobs/1"
      persistence={persistence}
    />
  );
}

describe('TailorFlow — F1 mount-level regression (DocumentsTab-shaped host)', () => {
  it('calls setRun exactly once for one run, and does not crash, under an unstable persistence object', async () => {
    const user = userEvent.setup();
    const onSetRun = vi.fn();
    const client = createMockClient({
      'resumePipeline.run': vi.fn().mockResolvedValue({ runId: 'run-1', jobId: 'job-1' }),
      'resumePipeline.get': vi.fn().mockResolvedValue(completedDetail()),
    });

    render(<DocumentsTabShapedHost onSetRun={onSetRun} />, { wrapper: withProviders(client) });

    await user.click(screen.getByTestId(TEST_IDS.documents.wizardGenerate));

    expect(await screen.findByTestId(TEST_IDS.documents.resultsPanel)).toBeInTheDocument();
    expect(onSetRun).toHaveBeenCalledTimes(1);
    expect(onSetRun).toHaveBeenCalledWith({ runId: 'run-1', jobId: 'job-1' });
  });
});
