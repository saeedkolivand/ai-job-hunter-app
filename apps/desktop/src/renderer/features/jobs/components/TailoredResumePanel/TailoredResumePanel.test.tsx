/**
 * TailoredResumePanel — the staged pipeline's ENTRY POINT.
 *
 * What is pinned here (each one is a behaviour someone could "simplify" away):
 *
 *  - **Fast ROUTES, it does not re-implement.** Start at `fast` calls the pane's
 *    existing tailor handler and never touches `resumePipeline.run`; quality does
 *    the opposite. Break the routing either way and both halves fail.
 *  - **The run request carries identity + inputs and nothing else.** No provider,
 *    model, base URL or budget field may appear — those are backend-owned — and
 *    `depth` is the SELECTED staged depth (Phase 4: `max` runs too), never a
 *    hardcoded one that would silently run a different pipeline than the one the
 *    control claims.
 *  - **Terminal state is the machine's, never the draft's.** A busy session with
 *    a full draft still renders as running with its display-only caption.
 *  - **`needsReview` is not a finish** — it gets its own count-carrying headline
 *    and never the completed copy.
 *  - **A nullable `stoppedReason`** renders no reason at all, never "Finished".
 *  - **An older run is read-only** (every run of a posting shares one saved
 *    résumé) and says why; a refusal that still comes back from the backend is
 *    surfaced verbatim rather than swallowed.
 *
 * Real `@ajh/ui`, real translations and the real report panel run — only the
 * session hook, the service layer and the posting-action handler are stubbed,
 * because those are the seams this component is wiring together.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { AgentStepEvent, JobEvent, JobRecord } from '@ajh/shared';
import type { ContentReportPayload, PipelineRunDetail, PipelineRunSummary } from '@ajh/shared/ipc';

import type { ResumePipelineSession } from '@/hooks/use-resume-pipeline-session';
import type { GenerationDepth } from '@/lib/generate';

import type { Posting } from '../../types';

interface MutationStub {
  mutate: ReturnType<typeof vi.fn>;
  isPending: boolean;
  variables?: { sectionKey?: string; issueKey?: string };
  error: unknown;
}

interface TestBus {
  session: ResumePipelineSession;
  depth: GenerationDepth;
  resume: { id: string; name: string } | null;
  config: { provider: string; model: string; effort?: string };
  runs: PipelineRunSummary[];
  details: Record<string, PipelineRunDetail | null>;
  regenerate: MutationStub;
  resolve: MutationStub;
  handleTailor: ReturnType<typeof vi.fn>;
  // ── the agent run behind "Improve this résumé" ────────────────────────────
  agentRun: ReturnType<typeof vi.fn>;
  cancelJob: ReturnType<typeof vi.fn>;
  agentConfirm: ReturnType<typeof vi.fn>;
  /** What the session asked React Query to refetch — the proof that an approved
   *  review reaches this panel instead of leaving it on the pre-save document. */
  invalidate: ReturnType<typeof vi.fn>;
  /** The live `agent:step` / `jobs:event` subscribers, so a test can drive a
   *  run the way the backend does. */
  onStep?: (event: AgentStepEvent) => void;
  onJobEvent?: (event: JobEvent) => void;
  /** Drives the reconciliation fallback — undefined unless a test needs it. */
  jobRecord?: JobRecord;
}

const bus = vi.hoisted((): TestBus => ({
  // Replaced in `beforeEach` before anything renders — the mock factory below
  // only closes over `bus`, it never reads it at import time.
  session: null as unknown as ResumePipelineSession,
  depth: 'quality',
  resume: { id: 'doc-1', name: 'ada-lovelace.pdf' },
  config: { provider: 'openai', model: 'gpt-5', effort: 'high' },
  runs: [],
  details: {},
  regenerate: { mutate: vi.fn(), isPending: false, error: null },
  resolve: { mutate: vi.fn(), isPending: false, error: null },
  handleTailor: vi.fn(),
  agentRun: vi.fn(),
  cancelJob: vi.fn(),
  agentConfirm: vi.fn(),
  invalidate: vi.fn(),
}));

// Mocked rather than wrapped in a real provider: `no-restricted-imports` bans
// importing `QueryClient`/`QueryClientProvider` inside `features/**` (the Ports
// & Adapters boundary), and the assertion here is about WHICH keys the session
// invalidates, which the stub reports exactly.
vi.mock('@tanstack/react-query', async (importOriginal) => {
  // Everything else stays real (`@/services/query-client` builds a `QueryClient`
  // at import time); only the hook the session uses is swapped. Typed through
  // the generic rather than the module's own types — even a type-only import of
  // those names trips the same boundary rule — and as a type ARGUMENT rather
  // than an assertion, which `no-unnecessary-type-assertion` autofixes away
  // (leaving a spread of `unknown` that only `tsc` catches).
  const actual = await importOriginal<Record<string, unknown>>();
  return { ...actual, useQueryClient: () => ({ invalidateQueries: bus.invalidate }) };
});

vi.mock('@/hooks/use-resume-pipeline-session', () => ({
  useResumePipelineSession: () => bus.session,
}));

vi.mock('@/hooks/useDefaultResumeId', () => ({
  useDefaultResume: () => bus.resume,
  useDefaultResumeId: () => bus.resume?.id ?? null,
}));

vi.mock('@/store/preferences-store', () => ({ useGenerationDepth: () => bus.depth }));

vi.mock('@/features/jobs/hooks/usePostingActions', () => ({
  usePostingActions: () => ({ handleTailor: bus.handleTailor }),
}));

vi.mock('@/services', () => ({
  useGenerateConfig: () => bus.config,
  usePipelineRun: (runId: string | null | undefined) => ({
    data: runId ? (bus.details[runId] ?? null) : undefined,
  }),
  usePipelineRunsForJob: () => ({ data: bus.runs }),
  useRegenerateSection: () => bus.regenerate,
  useResolveFabrication: () => bus.resolve,
  // The agent-run session behind the improve entry (`useAgentRunSession`).
  useAgentRun: () => ({ mutateAsync: bus.agentRun, isPending: false }),
  useAgentConfirm: () => ({ mutateAsync: bus.agentConfirm, isPending: false }),
  useAgentStepEvents: (cb: (event: AgentStepEvent) => void) => {
    bus.onStep = cb;
  },
  useCancelJob: () => ({ mutateAsync: bus.cancelJob }),
  useJob: () => ({ data: bus.jobRecord }),
  useJobEvents: (cb: (event: JobEvent) => void) => {
    bus.onJobEvent = cb;
  },
}));

import { canImproveGeneration, TailoredResumePanel } from './index';

const POSTING: Posting = {
  id: 'posting-1',
  source: 'linkedin',
  externalId: 'posting-1',
  url: 'https://example.test/job/1',
  title: 'Senior Engineer',
  company: 'Acme',
  description:
    'We are looking for a senior platform engineer to own our deployment pipeline and mentor the team.',
  capturedAt: 0,
};

const METRICS: ContentReportPayload['metrics'] = {
  keywordCoverage: 61,
  topRequirementHits: 2,
  topRequirementsMeasured: 3,
  duplicateRatio: 0,
  rolesSource: 2,
  rolesOutput: 2,
};

const START = vi.fn();
const CANCEL = vi.fn();

function makeSession(overrides: Partial<ResumePipelineSession> = {}): ResumePipelineSession {
  return {
    state: 'idle',
    busy: false,
    runId: null,
    jobId: null,
    stage: null,
    sectionStates: {},
    draft: '',
    detail: null,
    error: null,
    starting: false,
    start: START,
    cancel: CANCEL,
    reset: vi.fn(),
    ...overrides,
  };
}

function detail(overrides: Partial<PipelineRunDetail> = {}): PipelineRunDetail {
  return {
    runId: 'run-1',
    jobUrl: POSTING.url,
    kind: 'resume',
    depth: 'quality',
    status: 'completed',
    startedAt: Date.now() - 90_000,
    finishedAt: Date.now(),
    stoppedReason: 'done',
    metrics: { calls: 4, repairRounds: 1 },
    events: [],
    report: {
      schemaVersion: 2,
      pipeline: 'quality',
      generatedAt: Date.now(),
      resume: {
        sourceTextHash: 1,
        report: { ok: true, issues: [], metrics: METRICS },
      },
    },
    resumeText: 'Summary\nBuilt the deployment pipeline.\n\nExperience\nAcme',
    ...overrides,
  };
}

function summary(overrides: Partial<PipelineRunSummary> = {}): PipelineRunSummary {
  return {
    runId: 'run-1',
    jobUrl: POSTING.url,
    kind: 'resume',
    depth: 'quality',
    status: 'completed',
    startedAt: Date.now() - 90_000,
    stoppedReason: 'done',
    metrics: { calls: 4, repairRounds: 1 },
    ...overrides,
  };
}

/** Render and open the modal — every assertion below lives inside it. */
async function openPanel() {
  render(<TailoredResumePanel posting={POSTING} />);
  await userEvent.click(screen.getByRole('button', { name: /tailored résumé/i }));
}

/** The query keys an invalidation was asked for, in call order. */
const invalidatedKeys = () =>
  bus.invalidate.mock.calls.map((call) =>
    JSON.stringify((call[0] as { queryKey: unknown }).queryKey)
  );

beforeEach(() => {
  vi.clearAllMocks();
  bus.invalidate = vi.fn();
  bus.session = makeSession();
  bus.depth = 'quality';
  bus.resume = { id: 'doc-1', name: 'ada-lovelace.pdf' };
  bus.config = { provider: 'openai', model: 'gpt-5', effort: 'high' };
  bus.runs = [];
  bus.details = {};
  bus.regenerate = { mutate: vi.fn(), isPending: false, error: null };
  bus.resolve = { mutate: vi.fn(), isPending: false, error: null };
  bus.agentRun = vi.fn().mockResolvedValue({ jobId: 'agent-job-1' });
  bus.cancelJob = vi.fn().mockResolvedValue(undefined);
  bus.agentConfirm = vi.fn().mockResolvedValue({ ok: true });
  bus.onStep = undefined;
  bus.onJobEvent = undefined;
  bus.jobRecord = undefined;
  START.mockResolvedValue('run-1');
});

describe('TailoredResumePanel — idle', () => {
  it('offers the depth control and names the résumé it will actually use', async () => {
    await openPanel();
    expect(screen.getByRole('radiogroup', { name: /generation depth/i })).toBeInTheDocument();
    expect(screen.getByText(/ada-lovelace\.pdf/)).toBeInTheDocument();
  });

  it('says a résumé must be saved first, and refuses to start without one', async () => {
    bus.resume = null;
    await openPanel();
    const note = screen.getByText(/save a résumé first/i);
    const start = screen.getByRole('button', { name: /^start$/i });
    expect(start).toBeDisabled();
    // The disabled button POINTS at the reason rather than being silently dead.
    expect(start).toHaveAttribute('aria-describedby', note.id);
    await userEvent.click(start);
    expect(START).not.toHaveBeenCalled();
  });

  it('says a provider must be configured, and refuses to start without one', async () => {
    bus.config = { provider: '', model: '' };
    await openPanel();
    expect(screen.getByText(/choose an ai provider/i)).toBeInTheDocument();
    const start = screen.getByRole('button', { name: /^start$/i });
    expect(start).toBeDisabled();
    await userEvent.click(start);
    expect(START).not.toHaveBeenCalled();
  });
});

// ── The routing rule ────────────────────────────────────────────────────────
//
// `fast` is the existing single-shot flow and must be ENTERED, not rebuilt:
// point Start at `session.start` for fast (or at the tailor handler for
// quality) and one of these two tests fails.
describe('TailoredResumePanel — depth routing', () => {
  it('routes fast depth to the existing tailoring flow and starts no pipeline run', async () => {
    bus.depth = 'fast';
    await openPanel();
    expect(screen.getByText(/one-shot tailoring flow/i)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /^start$/i }));
    expect(bus.handleTailor).toHaveBeenCalledTimes(1);
    expect(START).not.toHaveBeenCalled();
  });

  it('runs the staged pipeline at quality depth and never touches the fast flow', async () => {
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /^start$/i }));
    expect(START).toHaveBeenCalledTimes(1);
    expect(bus.handleTailor).not.toHaveBeenCalled();
  });

  // Phase 4. Hardcode the request's `depth` back to `'quality'` (what this
  // surface used to send, because the backend rejected `max`) and this fails:
  // the control would say Max while a quality run went to the wire.
  it('sends the depth the user actually picked, so max runs the max pipeline', async () => {
    bus.depth = 'max';
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /^start$/i }));
    expect(START).toHaveBeenCalledTimes(1);
    expect((START.mock.calls[0]?.[0] as Record<string, unknown>).depth).toBe('max');
    expect(bus.handleTailor).not.toHaveBeenCalled();
  });

  it('sends identity + inputs only — never routing, budget or document text', async () => {
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /^start$/i }));

    const req = START.mock.calls[0]?.[0] as Record<string, unknown>;
    expect(req).toEqual({
      // Both ids are resolved SERVER-side from these, which is the whole reason
      // this surface — and not a wizard holding free text — may start a run.
      resumeId: 'doc-1',
      jobId: 'posting-1',
      jobUrl: POSTING.url,
      // The control's value (the bus is at `quality` here) — see the max case
      // above for the other half of that.
      depth: 'quality',
      targetLanguage: 'en',
      topRequirements: [],
      coverLetterText: '',
      effort: 'high',
    });
    for (const forbidden of [
      'provider',
      'model',
      'baseUrl',
      'maxSteps',
      'maxTokens',
      'runTimeout',
      'resumeText',
      'jobText',
    ]) {
      expect(req).not.toHaveProperty(forbidden);
    }
  });

  it('omits effort entirely when the provider has none set', async () => {
    bus.config = { provider: 'ollama', model: 'llama3' };
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /^start$/i }));
    expect(START.mock.calls[0]?.[0]).not.toHaveProperty('effort');
  });
});

describe('TailoredResumePanel — a live run', () => {
  // The record HAS landed and says `running` — that is the live shape, and it
  // is what makes the "is this terminal?" question answerable at all: a mutant
  // that reads the draft (or the record's presence) as the finish would have a
  // document and a report to render here.
  const running = () =>
    makeSession({
      state: 'drafting',
      busy: true,
      runId: 'run-1',
      jobId: 'job-1',
      stage: { stage: 'draft', phase: 'start', index: 3, total: 6, attempt: 1 },
      draft: 'Summary\nBuilt the deployment pipeline.',
      detail: detail({ status: 'running', stoppedReason: null, finishedAt: undefined }),
    });

  it('shows the stage, its counter and the display-only draft', async () => {
    bus.session = running();
    await openPanel();
    expect(screen.getByText('Writing the résumé')).toBeInTheDocument();
    expect(screen.getByText('Step 4 of 6')).toBeInTheDocument();
    expect(screen.getByText(/display only/i)).toBeInTheDocument();
    expect(screen.getByText(/Built the deployment pipeline/)).toBeInTheDocument();
  });

  // The draft stream completes its umbrella job several stages before the run
  // ends. A surface that reads "the draft arrived" as "the run finished" shows
  // an unvalidated, unrepaired document as final — so a busy session stays busy
  // no matter how complete the draft looks.
  it('stays on the running view while the machine is busy, whatever the draft says', async () => {
    bus.session = running();
    await openPanel();
    expect(screen.queryByText('Completed')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /open integrity report/i })
    ).not.toBeInTheDocument();
    // The record already carries a document; presenting it as the finished
    // résumé mid-run is the exact misreport.
    expect(screen.queryByText('Finished résumé')).not.toBeInTheDocument();
  });

  it('cancels through the session and locks the button afterwards', async () => {
    bus.session = running();
    await openPanel();
    const stop = screen.getByRole('button', { name: /stop run/i });
    await userEvent.click(stop);
    expect(CANCEL).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: /stopping/i })).toBeDisabled();
  });

  // ── The max-depth section timeline + progressive assembly ─────────────────
  it('shows the per-section checklist and the progressively assembled text', async () => {
    bus.depth = 'max';
    bus.session = makeSession({
      ...running(),
      stage: { stage: 'sections', phase: 'start', index: 3, total: 8, attempt: 1 },
      sectionStates: { summary: 'done', 'experience:0': 'generating' },
      // At max depth the same display-only stream carries whole SECTIONS as
      // `assemble` renders them, rather than draft tokens. The pane needs no
      // branch for that — which is what this asserts.
      draft: 'Summary\nBuilt the deployment pipeline.\n\nExperience\nAcme — Senior Engineer',
      detail: detail({ status: 'running', stoppedReason: null, finishedAt: undefined }),
    });
    await openPanel();
    expect(screen.getByText('Writing the résumé section by section')).toBeInTheDocument();
    expect(screen.getByText('Step 4 of 8')).toBeInTheDocument();
    const timeline = screen.getByTestId('section-timeline');
    expect(within(timeline).getByText('Summary')).toBeInTheDocument();
    expect(within(timeline).getByText('Experience 1')).toBeInTheDocument();
    expect(screen.getByText(/Acme — Senior Engineer/)).toBeInTheDocument();
    // Still display-only: the assembled text is not the finished résumé.
    expect(screen.getByText(/display only/i)).toBeInTheDocument();
    expect(screen.queryByText('Finished résumé')).not.toBeInTheDocument();
  });

  // Quality depth reports no sections at all, so the checklist must not leave
  // an empty captioned box behind on the surface that hosts it unconditionally.
  it('shows no checklist for a quality run', async () => {
    bus.session = running();
    await openPanel();
    expect(screen.queryByTestId('section-timeline')).toBeNull();
  });
});

describe('TailoredResumePanel — terminal states', () => {
  it('renders a completed run with its report action', async () => {
    bus.session = makeSession({ state: 'done', runId: 'run-1', detail: detail() });
    await openPanel();
    expect(screen.getByText('Completed')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /open integrity report/i })).toBeInTheDocument();
  });

  // `needsReview` is a document that exists and carries findings — never a
  // finish. Collapse it into `done` and this fails twice.
  it('presents needsReview as NOT done, with the open-claim count', async () => {
    bus.session = makeSession({
      state: 'needsReview',
      runId: 'run-1',
      detail: detail({
        status: 'needsReview',
        report: {
          schemaVersion: 2,
          pipeline: 'quality',
          generatedAt: 1,
          resume: {
            sourceTextHash: 1,
            report: { ok: false, issues: [], metrics: METRICS },
            fabrications: [
              { issueKey: 'factual.unsourced_metric#0', code: 'f', evidence: 'Built the' },
              {
                issueKey: 'factual.unsourced_metric#1',
                code: 'f',
                evidence: 'deployment pipeline',
              },
            ],
          },
        },
      }),
    });
    await openPanel();
    expect(screen.getByText(/2 claims need your verdict/i)).toBeInTheDocument();
    expect(screen.queryByText('Completed')).not.toBeInTheDocument();
  });

  it('surfaces a failed run with the honest "refused, not queued" copy', async () => {
    bus.session = makeSession({ state: 'error', error: 'job not found in cache: posting-1' });
    await openPanel();
    expect(screen.getByRole('alert')).toHaveTextContent(/job not found in cache/i);
    expect(screen.getByText(/refused rather than queued/i)).toBeInTheDocument();
  });

  // `stoppedReason` is nullable BY CONTRACT on a terminal run, and `'done'` is
  // never a valid stand-in: `status` is the authority.
  describe('a nullable stoppedReason', () => {
    // The positive control: without it, "renders nothing" would also pass on a
    // surface that renders no reason line at all, ever.
    it('renders the reason a terminal run DID record', async () => {
      bus.session = makeSession({
        state: 'error',
        runId: 'run-1',
        detail: detail({ status: 'failed', stoppedReason: 'run_timeout' }),
      });
      await openPanel();
      expect(screen.getByText(/ran out of time/i)).toBeInTheDocument();
    });

    it('renders no reason at all when the run recorded none — never "Finished"', async () => {
      bus.session = makeSession({
        state: 'error',
        runId: 'run-1',
        detail: detail({ status: 'failed', stoppedReason: null }),
      });
      await openPanel();
      expect(screen.getByText('Failed')).toBeInTheDocument();
      expect(screen.queryByText('Finished')).not.toBeInTheDocument();
      // Not a raw key either: a fallback that renders `pipeline.stopped.null`
      // is the same bug wearing a different label.
      expect(screen.queryByText(/pipeline\.stopped/)).not.toBeInTheDocument();
    });
  });
});

describe('TailoredResumePanel — the report', () => {
  it('opens the full report with verdicts and the per-bullet review', async () => {
    const withClaims = detail({
      status: 'needsReview',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 1,
        resume: {
          sourceTextHash: 1,
          report: { ok: false, issues: [], metrics: METRICS },
          fabrications: [
            { issueKey: 'factual.unsourced_metric#0', code: 'f', evidence: 'Built the' },
          ],
        },
      },
    });
    bus.session = makeSession({ state: 'needsReview', runId: 'run-1', detail: withClaims });
    bus.runs = [summary({ status: 'needsReview' })];
    bus.details = { 'run-1': withClaims };

    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /open integrity report/i }));

    expect(screen.getByText('Section verdicts')).toBeInTheDocument();
    expect(screen.getByText('Claims to review')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^remove$/i })).toBeInTheDocument();
    // The provenance line names the résumé the run actually started from.
    expect(screen.getByText(/ada-lovelace\.pdf/)).toBeInTheDocument();
  });

  it('records a Keep verdict against the run it is showing', async () => {
    const withClaims = detail({
      status: 'needsReview',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 1,
        resume: {
          sourceTextHash: 1,
          report: { ok: false, issues: [], metrics: METRICS },
          fabrications: [
            { issueKey: 'factual.unsourced_metric#0', code: 'f', evidence: 'Built the' },
          ],
        },
      },
    });
    bus.session = makeSession({ state: 'needsReview', runId: 'run-1', detail: withClaims });
    bus.runs = [summary({ status: 'needsReview' })];

    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /open integrity report/i }));
    await userEvent.click(screen.getByRole('button', { name: /^keep$/i }));

    expect(bus.resolve.mutate).toHaveBeenCalledWith({
      runId: 'run-1',
      issueKey: 'factual.unsourced_metric#0',
      decision: 'keep',
    });
  });

  // Every run of a posting merges into ONE saved résumé, so only the newest can
  // be written to. Offering buttons the backend is going to refuse is the thing
  // this withholding prevents — and the reason is said out loud.
  it('opens an older run read-only and says why nothing can be changed', async () => {
    const older = detail({
      runId: 'run-0',
      status: 'needsReview',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 1,
        resume: {
          sourceTextHash: 1,
          report: { ok: false, issues: [], metrics: METRICS },
          fabrications: [
            { issueKey: 'factual.unsourced_metric#0', code: 'f', evidence: 'Built the' },
          ],
        },
      },
    });
    bus.runs = [summary({ runId: 'run-1' }), summary({ runId: 'run-0' })];
    bus.details = { 'run-0': older };

    await openPanel();
    // The history list's own per-row action — the newest run is first.
    const [, olderOpen = null] = screen.getAllByRole('button', { name: /^open report$/i });
    if (!olderOpen) throw new Error('expected an open-report action on the older run');
    await userEvent.click(olderOpen);

    expect(screen.getByText(/not the newest run for this posting/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^remove$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /fix this section/i })).not.toBeInTheDocument();
  });

  /**
   * The race the read-only gate loses if it trusts the list alone.
   *
   * `resume_pipeline_run` returns its ids as soon as the run is ADMITTED; the
   * `pipeline_runs` row is written inside the spawned task, after admission and
   * after five resolutions. `start`'s invalidation therefore routinely refetches
   * a list whose newest entry is still the PREVIOUS run. Reading `runs[0]` alone
   * then declares the run the user is looking at "not the newest", withholds Fix
   * and Resolve, and strands it at `needsReview` with no way to clear it — while
   * telling the user, falsely, that they are looking at an older run.
   *
   * Mutation check: drop the `ownRun` term from `writable` and this fails three
   * times (no Remove, no Fix, and the older-run note appears).
   */
  it('keeps this session’s own run writable while the list still ends at the previous one', async () => {
    const mine = detail({
      runId: 'run-2',
      status: 'needsReview',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 1,
        resume: {
          sourceTextHash: 1,
          report: {
            ok: false,
            issues: [
              {
                code: 'ats.long_bullet',
                severity: 'warning',
                section: 'summary',
                message: 'This bullet is 340 characters.',
                evidence: 'Built the deployment pipeline.',
              },
            ],
            metrics: METRICS,
          },
          fabrications: [{ issueKey: 'factual.unsourced_metric#0', code: 'f', evidence: 'Built' }],
        },
      },
    });
    bus.session = makeSession({ state: 'needsReview', runId: 'run-2', detail: mine });
    // The stale list: the row for `run-2` has not landed yet.
    bus.runs = [summary({ runId: 'run-1' })];

    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /open integrity report/i }));

    expect(screen.getByRole('button', { name: /^remove$/i })).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: /fix this section/i }).length).toBeGreaterThan(0);
    expect(screen.queryByText(/not the newest run for this posting/i)).not.toBeInTheDocument();
  });

  // The other side of the same gate: an older run really is read-only, and the
  // session's own id must not launder it into a writable one.
  it('still refuses a genuinely older run, even from the same session', async () => {
    const older = detail({
      runId: 'run-0',
      status: 'needsReview',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 1,
        resume: {
          sourceTextHash: 1,
          report: { ok: false, issues: [], metrics: METRICS },
          fabrications: [{ issueKey: 'factual.unsourced_metric#0', code: 'f', evidence: 'Built' }],
        },
      },
    });
    bus.session = makeSession({ state: 'needsReview', runId: 'run-2', detail: detail() });
    bus.runs = [summary({ runId: 'run-2' }), summary({ runId: 'run-0' })];
    bus.details = { 'run-0': older };

    await openPanel();
    const [, olderOpen = null] = screen.getAllByRole('button', { name: /^open report$/i });
    if (!olderOpen) throw new Error('expected an open-report action on the older run');
    await userEvent.click(olderOpen);

    expect(screen.getByText(/not the newest run for this posting/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^remove$/i })).not.toBeInTheDocument();
  });

  // A refusal the backend DID return is never swallowed or auto-retried.
  it('surfaces a regenerate refusal verbatim instead of retrying it', async () => {
    bus.session = makeSession({ state: 'done', runId: 'run-1', detail: detail() });
    bus.regenerate = {
      mutate: vi.fn(),
      isPending: false,
      variables: undefined,
      error: new Error('There is a newer run for this posting'),
    };
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /open integrity report/i }));
    expect(screen.getByRole('alert')).toHaveTextContent(/there is a newer run for this posting/i);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// "Improve this résumé" — the `improve_resume` flow's only entry point.
//
// Every rule below withholds the action rather than offering one the backend
// would refuse: no generation to review, an older run, no résumé/provider, a
// generation too long to be read whole. The refusal states are visible, not
// silent.
// ─────────────────────────────────────────────────────────────────────────────

/** A finished run whose posting HAS a generated résumé — the shape the improve
 *  entry needs (`resumeText` is `find_for_job`'s record, the same document the
 *  run resolves server-side). */
function finishedWithDocument(resumeText = 'Summary\nBuilt the deployment pipeline.') {
  bus.session = makeSession({
    state: 'done',
    runId: 'run-1',
    detail: detail({ resumeText }),
  });
  bus.runs = [summary({ runId: 'run-1' })];
}

const improveButton = () => screen.queryByRole('button', { name: /improve this résumé/i });

// ─────────────────────────────────────────────────────────────────────────────
// The eligibility rule, term by term.
//
// The component cannot exercise all of it: opening another run's report
// REPLACES this modal (one dialog at a time), so every state that makes
// `writable` false also unmounts the footer the button lives in — a "no button"
// assertion there passes with or without the term. Asserted directly instead.
// ─────────────────────────────────────────────────────────────────────────────

describe('canImproveGeneration', () => {
  const eligible = {
    terminal: true,
    runState: 'done',
    hasDetail: true,
    hasDocument: true,
    writable: true,
    canRunStaged: true,
  };

  it('allows a finished run of the newest generated résumé', () => {
    expect(canImproveGeneration(eligible)).toBe(true);
    expect(canImproveGeneration({ ...eligible, runState: 'needsReview' })).toBe(true);
  });

  it.each([
    ['still running', { terminal: false }],
    ['a failed run', { runState: 'error' }],
    ['a stopped run', { runState: 'cancelled' }],
    ['no run record', { hasDetail: false }],
    ['no generated document', { hasDocument: false }],
    // The one the component test cannot reach: every run of a posting merges
    // into ONE saved résumé, so an older run must not offer a review whose save
    // would land on the newest document.
    ['an older run than the newest', { writable: false }],
    ['no résumé or provider', { canRunStaged: false }],
  ])('refuses %s', (_case, override) => {
    expect(canImproveGeneration({ ...eligible, ...override })).toBe(false);
  });
});

describe('TailoredResumePanel — the improve entry', () => {
  it('offers the action on a finished run of the newest generated résumé', async () => {
    finishedWithDocument();
    await openPanel();
    expect(improveButton()).toBeEnabled();
  });

  it('starts the improve flow with the master résumé id and nothing else', async () => {
    finishedWithDocument();
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));

    expect(bus.agentRun).toHaveBeenCalledTimes(1);
    // `resumeId` is the candidate's MASTER résumé (the ground truth claims are
    // checked against), never the generation's id; the document under review is
    // resolved server-side from `jobId`. `kind` is stated, never defaulted.
    expect(bus.agentRun).toHaveBeenCalledWith({
      resumeId: 'doc-1',
      jobId: 'posting-1',
      kind: 'improve_resume',
    });
    // The run's own progress lands on this surface, not somewhere else.
    expect(screen.getByRole('region', { name: /improving this résumé/i })).toBeInTheDocument();
  });

  it('withholds it while the pipeline is still running', async () => {
    bus.session = makeSession({
      state: 'drafting',
      busy: true,
      runId: 'run-1',
      draft: 'Summary',
      detail: detail({ status: 'running', stoppedReason: null, finishedAt: undefined }),
    });
    await openPanel();
    expect(improveButton()).toBeNull();
  });

  // No generation for this posting means the run fails at "generate one first"
  // — not offering it is the honest answer.
  it('withholds it when the run produced no document to review', async () => {
    bus.session = makeSession({
      state: 'done',
      runId: 'run-1',
      detail: detail({ resumeText: '' }),
    });
    await openPanel();
    expect(improveButton()).toBeNull();
  });

  // The improve flow's save merges into the SAME one-document-per-posting
  // aggregate the pipeline's write actions do, so it carries the same
  // newest-run rule (`writable`). What is observable from here is the composite:
  // an older run is shown in the report, which REPLACES this modal (one dialog
  // at a time), so no improve entry is reachable while looking at one — the
  // `writable` term is belt-and-braces behind that, for a layout that ever
  // renders this footer next to another run.
  it('offers no improve entry while an older run is the one on screen', async () => {
    const older = detail({ runId: 'run-0' });
    bus.session = makeSession();
    bus.runs = [summary({ runId: 'run-1' }), summary({ runId: 'run-0' })];
    bus.details = { 'run-0': older };

    await openPanel();
    const [, olderOpen = null] = screen.getAllByRole('button', { name: /^open report$/i });
    if (!olderOpen) throw new Error('expected an open-report action on the older run');
    await userEvent.click(olderOpen);
    expect(improveButton()).toBeNull();
  });

  // A failed or stopped pipeline run leaves whatever it had written behind.
  // That is not a document a review can improve, and offering it invites a
  // paid run over a fragment.
  it.each(['error', 'cancelled'] as const)(
    'withholds it on a %s run, whatever it left behind',
    async (state) => {
      bus.session = makeSession({ state, runId: 'run-1', detail: detail() });
      bus.runs = [summary({ runId: 'run-1' })];
      await openPanel();
      expect(improveButton()).toBeNull();
    }
  );

  // needsReview IS a document — the open claims are exactly what a review helps
  // with, so this is the case the entry exists for.
  it('offers it on a needsReview run, which is a document with open claims', async () => {
    bus.session = makeSession({ state: 'needsReview', runId: 'run-1', detail: detail() });
    bus.runs = [summary({ runId: 'run-1' })];
    await openPanel();
    expect(improveButton()).toBeEnabled();
  });

  // The review is about to offer the CURRENT document back through its gated
  // save; a generation started underneath it would be overwritten by the older
  // reviewed text on approve.
  it('locks the pipeline’s own Run again while a review is in flight', async () => {
    finishedWithDocument();
    await openPanel();
    expect(screen.getByRole('button', { name: /run again/i })).toBeEnabled();

    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));
    expect(screen.getByRole('button', { name: /run again/i })).toBeDisabled();
  });

  // A suspended confirm is invisible once the modal is closed, and it expires
  // in about five minutes — the closed state has to carry that.
  it('marks the closed panel when a review is waiting on the user', async () => {
    finishedWithDocument();
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));
    await act(async () => {
      bus.onStep?.({
        jobId: 'agent-job-1',
        step: 6,
        text: '',
        tools: [],
        denied: [],
        kind: 'confirm_request',
        confirm: { callId: '6-0-save_resume', tool: 'save_resume', args: { resumeText: 'Fixed.' } },
      });
    });

    const trigger = screen.getByRole('button', { name: /tailored résumé/i });
    // Visible WORDS, not just the amber dot: colour alone fails WCAG 1.4.1 for
    // a sighted user who is not running a screen reader.
    expect(within(trigger).getByText('Approval needed')).toBeInTheDocument();
    // …and the sr-only sentence still carries the expiry.
    expect(trigger).toHaveTextContent(/waiting for your approval/i);
    expect(trigger).toHaveTextContent(/five minutes/i);
  });

  it('withholds it without a provider to run it with', async () => {
    finishedWithDocument();
    bus.config = { provider: '', model: '' };
    await openPanel();
    expect(improveButton()).toBeNull();
  });

  it('withholds it without a saved résumé to check claims against', async () => {
    finishedWithDocument();
    bus.resume = null;
    await openPanel();
    expect(improveButton()).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// An approved save must reach the panel.
//
// The gated save writes the `ai_generations` aggregate — the SAME record this
// panel's run detail reads its résumé text and report from. Nothing invalidates
// it on its own: the run query has no interval once terminal, and the client's
// refetch-on-focus/-reconnect are off. Without these, the card says "saved"
// beside the pre-save text, in one viewport, forever.
// ─────────────────────────────────────────────────────────────────────────────

describe('TailoredResumePanel — after an approved review', () => {
  /** Start a run and suspend it on the gated save. */
  async function suspendOnSave() {
    finishedWithDocument();
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));
    await act(async () => {
      bus.onStep?.({
        jobId: 'agent-job-1',
        step: 6,
        text: '',
        tools: [],
        denied: [],
        kind: 'confirm_request',
        confirm: { callId: '6-0-save_resume', tool: 'save_resume', args: { resumeText: 'Fixed.' } },
      });
    });
  }

  it('refetches the saved document and the run detail when the user approves', async () => {
    await suspendOnSave();
    bus.invalidate.mockClear();

    await act(async () => {
      await userEvent.click(screen.getByRole('button', { name: /^approve$/i }));
    });

    expect(invalidatedKeys()).toEqual(
      expect.arrayContaining([JSON.stringify(['pipeline']), JSON.stringify(['aiGenerations'])])
    );
  });

  // The write executes AFTER `agent.confirm` returns, so the refetch fired on
  // approve can race it — the next step is proof the tool finished.
  it('refetches again on the next step, after the write has actually landed', async () => {
    await suspendOnSave();
    await act(async () => {
      await userEvent.click(screen.getByRole('button', { name: /^approve$/i }));
    });
    bus.invalidate.mockClear();

    await act(async () => {
      bus.onStep?.({
        jobId: 'agent-job-1',
        step: 7,
        text: 'Saved.',
        tools: [],
        denied: [],
        kind: 'proposal',
      });
    });

    expect(invalidatedKeys()).toEqual(
      expect.arrayContaining([JSON.stringify(['pipeline']), JSON.stringify(['aiGenerations'])])
    );
  });

  it('refetches nothing when the user denies — a denial changed nothing', async () => {
    await suspendOnSave();
    bus.invalidate.mockClear();

    await act(async () => {
      await userEvent.click(screen.getByRole('button', { name: /^deny$/i }));
    });

    expect(invalidatedKeys()).toEqual([]);
  });
});

describe('TailoredResumePanel — a generation too long to review', () => {
  // The seed fences the generation at 8 000 characters. Reviewing a truncated
  // copy and then offering it back through `save_resume` (cap 40 000) would
  // overwrite the document with its own head, so the backend refuses the run —
  // and this surface refuses the CLICK, with the reason said out loud.
  const TOO_LONG = 'x'.repeat(8_001);
  /** One code point, two UTF-16 units — the difference the gate turns on. */
  const ASTRAL = String.fromCodePoint(0x1f600);

  // `aria-disabled`, not `disabled`: a natively disabled button leaves the tab
  // order (so its `aria-describedby` is never announced) and this repo's Button
  // kills pointer events on it (so a `title` on a NATIVELY disabled control
  // never fires). `aria-disabled` keeps both, which is the point — and is why
  // the tooltip below has to match the state rather than describe the action.
  it('keeps the action focusable, explains itself, and refuses the click', async () => {
    finishedWithDocument(TOO_LONG);
    await openPanel();

    const button = improveButton();
    expect(button).not.toBeDisabled();
    expect(button).toHaveAttribute('aria-disabled', 'true');
    const note = screen.getByText(/longer than the review can read/i);
    expect(button).toHaveAttribute('aria-describedby', note.id);
    // Reachable by keyboard, which is what makes the description announceable.
    button?.focus();
    expect(document.activeElement).toBe(button);

    if (button) await userEvent.click(button);
    expect(bus.agentRun).not.toHaveBeenCalled();
  });

  // With `aria-disabled` the tooltip DOES fire, so a generic "what this does"
  // hint on a button that will refuse the click is actively misleading.
  it('points its tooltip at the refusal, not at what the action would have done', async () => {
    finishedWithDocument(TOO_LONG);
    await openPanel();
    expect(improveButton()).toHaveAttribute('title', expect.stringMatching(/too long to review/i));
  });

  it('goes back to describing the action once the résumé fits', async () => {
    finishedWithDocument();
    await openPanel();
    expect(improveButton()).toHaveAttribute('title', expect.stringMatching(/re-check/i));
  });

  // The production check spreads the string specifically to match the Rust
  // `chars().take(RESUME_CAP)` clamp. Every other fixture here is ASCII, where
  // `String.length` and the code-point count agree — so a regression to
  // `.length` would ship green without this pair.
  it('counts code points, not UTF-16 units: 8 000 astral chars still fit', async () => {
    // `.length` is 16 000 here; the code-point count is exactly the cap.
    finishedWithDocument(ASTRAL.repeat(8_000));
    await openPanel();
    expect(improveButton()).toBeEnabled();
    expect(screen.queryByText(/longer than the review can read/i)).not.toBeInTheDocument();
  });

  it('still refuses one astral character past the cap', async () => {
    finishedWithDocument(ASTRAL.repeat(8_001));
    await openPanel();
    expect(improveButton()).toHaveAttribute('aria-disabled', 'true');
  });

  it('formats the cap for the locale rather than printing a bare number', async () => {
    finishedWithDocument(TOO_LONG);
    await openPanel();
    expect(screen.getByText(/8,000 characters/)).toBeInTheDocument();
  });

  // The note sits in the FOOTER, with the button — below the scrollable body it
  // was off-viewport from the control it explains.
  it('puts the reason in the footer, next to the action it refuses', async () => {
    finishedWithDocument(TOO_LONG);
    await openPanel();
    const note = screen.getByText(/longer than the review can read/i);
    const button = improveButton();
    expect(note.parentElement?.contains(button ?? null)).toBe(true);
  });

  it('still runs at exactly the cap — the refusal is "longer than", not "as long as"', async () => {
    finishedWithDocument('y'.repeat(8_000));
    await openPanel();
    expect(improveButton()).toBeEnabled();
    expect(screen.queryByText(/longer than the review can read/i)).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// A live review holds the document.
//
// It captured the text at run start and is about to offer a corrected version
// through `save_resume`, which replaces the aggregate WHOLESALE. A Fix or a
// Resolve landing in between is admitted by the limiter (the suspended run
// holds one of two slots) and then silently overwritten on approve, with no
// undo — so the report's writes are withheld for the duration, and say why.
// ─────────────────────────────────────────────────────────────────────────────

describe('TailoredResumePanel — the report during a live review', () => {
  const withClaims = () =>
    detail({
      status: 'needsReview',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 1,
        resume: {
          sourceTextHash: 1,
          report: {
            ok: false,
            issues: [
              {
                code: 'ats.long_bullet',
                severity: 'warning',
                section: 'summary',
                message: 'This bullet is 340 characters.',
                evidence: 'Built the deployment pipeline.',
              },
            ],
            metrics: METRICS,
          },
          fabrications: [
            { issueKey: 'factual.unsourced_metric#0', code: 'f', evidence: 'Built the' },
          ],
        },
      },
    });

  async function openReportDuringReview({ review }: { review: boolean }) {
    bus.session = makeSession({ state: 'needsReview', runId: 'run-1', detail: withClaims() });
    bus.runs = [summary({ runId: 'run-1', status: 'needsReview' })];
    await openPanel();
    if (review) {
      await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));
    }
    await userEvent.click(screen.getByRole('button', { name: /open integrity report/i }));
  }

  // The positive control: these actions exist when no review is running.
  it('offers Fix and Remove when nothing else holds the document', async () => {
    await openReportDuringReview({ review: false });
    expect(screen.getByRole('button', { name: /^remove$/i })).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: /fix this section/i }).length).toBeGreaterThan(0);
  });

  it('withholds both while a review is in flight, and says why', async () => {
    await openReportDuringReview({ review: true });
    expect(screen.queryByRole('button', { name: /^remove$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /fix this section/i })).not.toBeInTheDocument();
    expect(screen.getByText(/review of this résumé is running/i)).toBeInTheDocument();
    // …and not the older-run reason, which is a different fact.
    expect(screen.queryByText(/not the newest run for this posting/i)).not.toBeInTheDocument();
  });

  // Reading is not the hazard — only writing is.
  it('still opens the report itself', async () => {
    await openReportDuringReview({ review: true });
    expect(screen.getByText('Section verdicts')).toBeInTheDocument();
  });

  it('explains the pause in the footer too, where Run again is dead', async () => {
    finishedWithDocument();
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));
    expect(screen.getByText(/editing it from the report, are paused/i)).toBeInTheDocument();
  });
});

describe('TailoredResumePanel — an improve run that fails', () => {
  // The pre-check reads the document this surface was SHOWN; the stored text is
  // the authority, so a refusal that still comes back is surfaced verbatim
  // rather than leaving the card spinning.
  it('surfaces the backend refusal on the card instead of spinning', async () => {
    finishedWithDocument();
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));

    await act(async () => {
      bus.onJobEvent?.({
        jobId: 'agent-job-1',
        type: 'job.failed',
        data: 'this résumé is longer than the review flow can read — trim or regenerate it first',
      } as JobEvent);
    });

    expect(screen.getByRole('alert')).toHaveTextContent(/longer than the review flow can read/i);
    // A failed run is dismissable — the state has an action, not just a message.
    expect(screen.getByRole('button', { name: /dismiss/i })).toBeInTheDocument();
  });

  // The backend can fail an improve run BEFORE `agent.run`'s round-trip
  // resolves — before anything is listening — so the terminal event is dropped
  // and the card would spin forever. Refusing fast is this flow's normal
  // behaviour (no generation, no posting url, a generation too long), which
  // makes the record reconciliation the likely path here, not the rare one.
  it('reconciles a failure that beat the subscription, from the job record', async () => {
    finishedWithDocument();
    const failedRun: JobRecord = {
      id: 'agent-job-1',
      // The backend records this run as the free-form kind `"agent.run"`,
      // which the shared `JobKind` union does not carry; reconciliation reads
      // `status`/`error` only, so any member stands in for it here.
      kind: 'ai.generate',
      status: 'failed',
      progress: 0,
      payload: {},
      error: 'this job has no posting URL, so no generated résumé is linked to it',
      retries: 0,
      maxRetries: 0,
      createdAt: 0,
      updatedAt: 0,
    };
    bus.jobRecord = failedRun;

    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));

    // `find*`, not `get*`: reconciliation lands on the render AFTER the run id
    // does, so a synchronous read races the effect rather than testing it.
    expect(await screen.findByRole('alert')).toHaveTextContent(/no posting URL/i);
  });

  // `job.completed` carries the run's `stoppedReason`; a ceiling-stopped review
  // is not a finished one, and the panel must not launder it into success.
  it('keeps the stopped reason from a completed run', async () => {
    finishedWithDocument();
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));

    await act(async () => {
      bus.onJobEvent?.({
        jobId: 'agent-job-1',
        type: 'job.completed',
        data: { finalText: 'done', steps: 8, stoppedReason: 'max_steps' },
      } as JobEvent);
    });

    expect(await screen.findByText('Stopped at its step limit')).toBeInTheDocument();
  });

  // The card renders `failedTitle` as the alert heading and the message under
  // it; a fallback that reused the heading printed the same sentence twice.
  it('does not print the alert heading twice when the failure carries no message', async () => {
    finishedWithDocument();
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));

    await act(async () => {
      bus.onJobEvent?.({ jobId: 'agent-job-1', type: 'job.failed', data: '' } as JobEvent);
    });

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent(/The review failed/);
    expect(alert).toHaveTextContent(/stopped without saying why/);
    expect(alert.textContent?.match(/The review failed/g)).toHaveLength(1);
  });

  it('ignores a terminal event belonging to a different run', async () => {
    finishedWithDocument();
    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));

    await act(async () => {
      bus.onJobEvent?.({
        jobId: 'someone-elses-job',
        type: 'job.failed',
        data: 'nope',
      } as JobEvent);
    });

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByText(/starting the review/i)).toBeInTheDocument();
  });
});

// The record fallback is not a one-shot: `useJobEvents` invalidates `['jobs']`
// on EVERY job event, and prefix invalidation marks `['jobs', id]` stale, so
// this query refetches on each transition without a polling timer. What must
// hold is that a LATER reading still reconciles — the ref latches only once a
// terminal status has actually been seen.
describe('TailoredResumePanel — the record fallback keeps checking', () => {
  it('reconciles on a later reading, not just the first one', async () => {
    finishedWithDocument();
    const running: JobRecord = {
      id: 'agent-job-1',
      kind: 'ai.generate',
      status: 'running',
      progress: 0,
      payload: {},
      retries: 0,
      maxRetries: 0,
      createdAt: 0,
      updatedAt: 0,
    };
    bus.jobRecord = running;

    await openPanel();
    await userEvent.click(screen.getByRole('button', { name: /improve this résumé/i }));
    // A running record reconciles nothing — the run really is in flight.
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();

    // The record turns terminal while the terminal EVENT never arrives (the
    // case this fallback exists for). Any later reading has to pick it up — in
    // the app that reading comes from `useJobEvents` invalidating `['jobs']` on
    // every event; here, from the re-render a further step causes.
    bus.jobRecord = { ...running, status: 'failed', error: 'the model went away' };
    await act(async () => {
      bus.onStep?.({
        jobId: 'agent-job-1',
        step: 2,
        text: 'still narrating',
        tools: ['validate_resume'],
        denied: [],
        kind: 'turn',
      });
    });

    expect(await screen.findByRole('alert')).toHaveTextContent(/the model went away/);
  });
});

describe('TailoredResumePanel — run history', () => {
  it('lists the posting’s retained runs', async () => {
    bus.runs = [summary(), summary({ runId: 'run-0', status: 'cancelled', stoppedReason: null })];
    await openPanel();
    expect(screen.getByText(/staged runs for this job/i)).toBeInTheDocument();
    expect(screen.getByText('Cancelled')).toBeInTheDocument();
  });
});
