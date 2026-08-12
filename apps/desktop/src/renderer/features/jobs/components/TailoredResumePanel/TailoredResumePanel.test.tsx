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
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

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
}));

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
}));

import { TailoredResumePanel } from './index';

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

beforeEach(() => {
  vi.clearAllMocks();
  bus.session = makeSession();
  bus.depth = 'quality';
  bus.resume = { id: 'doc-1', name: 'ada-lovelace.pdf' };
  bus.config = { provider: 'openai', model: 'gpt-5', effort: 'high' };
  bus.runs = [];
  bus.details = {};
  bus.regenerate = { mutate: vi.fn(), isPending: false, error: null };
  bus.resolve = { mutate: vi.fn(), isPending: false, error: null };
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

describe('TailoredResumePanel — run history', () => {
  it('lists the posting’s retained runs', async () => {
    bus.runs = [summary(), summary({ runId: 'run-0', status: 'cancelled', stoppedReason: null })];
    await openPanel();
    expect(screen.getByText(/staged runs for this job/i)).toBeInTheDocument();
    expect(screen.getByText('Cancelled')).toBeInTheDocument();
  });
});
