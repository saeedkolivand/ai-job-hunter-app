import { createElement, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import type { AiGenerationRecord } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';

import type * as GenerateModule from '@/lib/generate';
import { exportDOCX, exportPDF } from '@/lib/generate';
import { keys } from '@/services/query-client';

import { resolveTargetLanguage, useTailorPipeline } from './useTailorPipeline';

// Echoes the key verbatim, EXCEPT: a key outside these two small "known"
// sets (mirroring the real `pipeline.stage.*`/`pipeline.state.*` catalog)
// falls back to `defaultValue` when the caller passes one — the real
// i18next missing-key contract, which `stageLabel`'s two call sites are the
// only ones in this hook to rely on. Every other `t(...)` call (no
// `defaultValue`, or a key that IS "known") is unaffected.
const KNOWN_I18N_KEYS = new Set([
  ...[
    'analyze_job',
    'match_evidence',
    'strategy',
    'draft',
    'cover_letter',
    'validate',
    'repair',
    'humanize',
  ].map((s) => `pipeline.stage.${s}`),
  ...['queued', 'preparing', 'drafting', 'validating', 'repairing', 'humanizing'].map(
    (s) => `pipeline.state.${s}`
  ),
]);
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, opts?: Record<string, unknown>) => {
      if (KNOWN_I18N_KEYS.has(k)) return k;
      return opts && 'defaultValue' in opts ? (opts.defaultValue as string) : k;
    },
  }),
}));

const mockNotify = { error: vi.fn() };
vi.mock('@ajh/ui', () => ({ useNotification: () => mockNotify }));

// ── Session — the seam this hook wraps. Fully controlled by the test. ────────

const sessionBus = vi.hoisted(() => ({
  state: 'idle',
  busy: false,
  runId: null as string | null,
  jobId: null as string | null,
  stage: null as { stage: string; phase: 'start' | 'finish' | 'error' } | null,
  draft: '',
  letterDraft: '',
  thinking: '',
  detail: null as PipelineRunDetail | null,
  error: null as string | null,
  starting: false,
  start: vi.fn(),
  cancel: vi.fn(),
  reset: vi.fn(),
}));

vi.mock('@/hooks/use-resume-pipeline-session', () => ({
  useResumePipelineSession: () => ({ ...sessionBus }),
}));

// ── Sibling service hooks — stub, capture calls ───────────────────────────────

const regenerateMutate = vi.fn();
const resolveFabricationMutate = vi.fn();
const updateAiGenerationMutate = vi.fn();

vi.mock('@/services/use-resume-pipeline', () => ({
  usePipelineRunsForJob: () => ({ data: [] }),
  useRegenerateSection: () => ({ mutate: regenerateMutate, isPending: false, error: null }),
  useResolveFabrication: () => ({
    mutate: resolveFabricationMutate,
    isPending: false,
    error: null,
  }),
}));

vi.mock('@/services/use-ai-generations', () => ({
  useUpdateAiGeneration: () => ({ mutate: updateAiGenerationMutate }),
}));

vi.mock('@/hooks/use-quality-recheck', () => ({
  useQualityRecheck: () => ({ recheck: undefined, rechecking: false }),
}));

vi.mock('@/lib/generate', async () => {
  const actual = await vi.importActual<typeof GenerateModule>('@/lib/generate');
  return {
    ...actual,
    buildFilename: vi.fn(() => 'file.pdf'),
    exportDOCX: vi.fn(),
    exportPDF: vi.fn(),
    exportTXT: vi.fn(),
  };
});

// A module-scoped, per-test-reset client (not a fresh one per `render()` call)
// so a test can `vi.spyOn` its `invalidateQueries` and observe what the hook
// under test does to it.
let queryClient: QueryClient;
const wrapper = ({ children }: { children: ReactNode }) =>
  createElement(QueryClientProvider, { client: queryClient }, children);

const PARAMS = {
  jobDesc: 'a very German-language job ad'.repeat(1), // language detection is best-effort; not asserted precisely
  sourceResume: 'my resume',
  jobUrl: 'https://acme.com/job/1',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  board: 'linkedin',
  canUse: true,
  hasDesc: true,
  templateId: 'classic' as const,
  atsMode: false,
};

function render(overrides: Partial<Parameters<typeof useTailorPipeline>[0]> = {}) {
  return renderHook(() => useTailorPipeline({ ...PARAMS, ...overrides }), { wrapper });
}

function detail(overrides: Partial<PipelineRunDetail> = {}): PipelineRunDetail {
  return {
    runId: 'run-1',
    jobUrl: PARAMS.jobUrl,
    kind: 'resume',
    depth: 'quality',
    status: 'completed',
    startedAt: 1,
    metrics: {},
    events: [],
    report: null,
    resumeText: 'FINAL RESUME',
    ...overrides,
  };
}

beforeEach(() => {
  queryClient = new QueryClient();
  sessionBus.state = 'idle';
  sessionBus.busy = false;
  sessionBus.runId = null;
  sessionBus.jobId = null;
  sessionBus.stage = null;
  sessionBus.draft = '';
  sessionBus.letterDraft = '';
  sessionBus.thinking = '';
  sessionBus.detail = null;
  sessionBus.error = null;
  sessionBus.starting = false;
  sessionBus.start.mockReset().mockResolvedValue('run-1');
  sessionBus.cancel.mockReset();
  regenerateMutate.mockClear();
  resolveFabricationMutate.mockClear();
  updateAiGenerationMutate.mockClear();
  mockNotify.error.mockClear();
  vi.mocked(exportPDF).mockClear();
  vi.mocked(exportDOCX).mockClear();
});

describe('useTailorPipeline — start() builds the id-wins run request', () => {
  it('sends resumeId (and an empty resumeText) when the wizard résumé is doc-backed', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.start({
        resume: 'the résumé text',
        resumeDocId: 'doc-42',
        outputType: 'both',
        researchCompany: false,
      });
    });

    expect(sessionBus.start).toHaveBeenCalledWith(
      expect.objectContaining({ resumeId: 'doc-42', resumeText: '' })
    );
  });

  it('sends resumeText (and an empty resumeId) when the résumé has no backing doc', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.start({
        resume: 'pasted résumé text',
        outputType: 'both',
        researchCompany: false,
      });
    });

    expect(sessionBus.start).toHaveBeenCalledWith(
      expect.objectContaining({ resumeId: '', resumeText: 'pasted résumé text' })
    );
  });

  it('sets includeCoverLetter from outputType', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(sessionBus.start).toHaveBeenCalledWith(
      expect.objectContaining({ includeCoverLetter: false })
    );

    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'cover', researchCompany: false });
    });
    expect(sessionBus.start).toHaveBeenLastCalledWith(
      expect.objectContaining({ includeCoverLetter: true })
    );
  });

  it('never fabricates a jobUrl — sends exactly what it was given, including empty', async () => {
    const { result } = render({ jobUrl: '' });
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(sessionBus.start).toHaveBeenCalledWith(expect.objectContaining({ jobUrl: '' }));
  });

  it('does not start a run when AI is unavailable or there is no job ad', async () => {
    const { result } = render({ canUse: false });
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(sessionBus.start).not.toHaveBeenCalled();
  });

  it('toasts a failed start — the session already set the persistent banner text', async () => {
    sessionBus.start.mockResolvedValueOnce(null);
    const { result } = render();
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(mockNotify.error).toHaveBeenCalledWith({ message: 'autopilot.apply.failed' });
  });

  it('does not toast a successful start', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(mockNotify.error).not.toHaveBeenCalled();
  });
});

// The gap this closes: `market`/`today`/`researchCompany` were added to
// `ResumePipelineRunSchema` (all `.default()`-ed) but `start()` never sent
// any of them, so they silently sat at their defaults ('intl', '', false)
// and the letter-market-conventions/date/company-research features they
// unlock were inert regardless of the wizard's own state. Real (unmocked)
// `detectLanguage` fixtures, lifted from the market describe block below.
describe('useTailorPipeline — start() sends market/today/researchCompany (letter-export contract)', () => {
  const GERMAN_JOB_AD =
    'Erfahrener Softwareentwickler mit fundierten Kenntnissen in der Entwicklung skalierbarer Webanwendungen und verteilter Backend-Systeme für große Unternehmen.';
  const ENGLISH_JOB_AD =
    'Experienced software engineer with a strong background in building scalable web applications and distributed backend systems for large organisations.';

  it('sends the German market for a German-language posting', async () => {
    const { result } = render({ jobDesc: GERMAN_JOB_AD });
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(sessionBus.start).toHaveBeenCalledWith(expect.objectContaining({ market: 'de' }));
  });

  it('sends the US market for a US-located English posting', async () => {
    const { result } = render({ jobDesc: ENGLISH_JOB_AD, jobLocation: 'New York, NY, US' });
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(sessionBus.start).toHaveBeenCalledWith(expect.objectContaining({ market: 'us' }));
  });

  it('sends a non-empty, German-formatted today for a German posting', async () => {
    const { result } = render({ jobDesc: GERMAN_JOB_AD });
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    // Derived with the SAME `toLocaleDateString` call `start()` uses — asserts
    // the shape/locale, not a hardcoded literal that would break tomorrow.
    const expectedToday = new Date().toLocaleDateString('de', {
      day: 'numeric',
      month: 'long',
      year: 'numeric',
    });
    expect(expectedToday).not.toBe('');
    expect(sessionBus.start).toHaveBeenCalledWith(
      expect.objectContaining({ today: expectedToday })
    );
  });

  it('reflects researchCompany: true from the wizard values', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: true });
    });
    expect(sessionBus.start).toHaveBeenCalledWith(
      expect.objectContaining({ researchCompany: true })
    );
  });

  it('reflects researchCompany: false from the wizard values', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(sessionBus.start).toHaveBeenCalledWith(
      expect.objectContaining({ researchCompany: false })
    );
  });
});

describe('useTailorPipeline — document text sources', () => {
  it('reads the résumé from the run detail and the letter from the aggregate record', () => {
    sessionBus.detail = detail({ resumeText: 'RESUME FROM RUN' });
    const generation = {
      id: 'gen-1',
      coverLetterText: 'LETTER FROM AGGREGATE',
    } as AiGenerationRecord;
    const { result } = render({ latestGeneration: generation });

    expect(result.current.resumeOut).toBe('RESUME FROM RUN');
    act(() => result.current.setActiveOut('cover'));
    expect(result.current.coverOut).toBe('LETTER FROM AGGREGATE');
    expect(result.current.output).toBe('LETTER FROM AGGREGATE');
  });

  it('hasOutput is false while idle with no detail and no aggregate letter', () => {
    const { result } = render();
    expect(result.current.hasOutput).toBe(false);
  });

  it('cold entry: falls back to the aggregate résumé text when no live run detail exists', () => {
    // No session.detail — a fresh session that never started/reconnected a
    // run, but the posting already has a saved result from elsewhere.
    const generation = {
      id: 'gen-1',
      resumeText: 'RESUME FROM A PAST RUN',
      coverLetterText: '',
    } as AiGenerationRecord;
    const { result } = render({ latestGeneration: generation });

    expect(result.current.resumeOut).toBe('RESUME FROM A PAST RUN');
    expect(result.current.hasOutput).toBe(true);
    expect(result.current.meta).not.toBeNull();
  });

  it('prefers the LIVE run detail over the aggregate once one exists', () => {
    sessionBus.detail = detail({ resumeText: 'LIVE RESUME' });
    const generation = {
      id: 'gen-1',
      resumeText: 'STALE RESUME',
      coverLetterText: '',
    } as AiGenerationRecord;
    const { result } = render({ latestGeneration: generation });

    expect(result.current.resumeOut).toBe('LIVE RESUME');
  });
});

describe('useTailorPipeline — inline edit persistence', () => {
  // CR-8: teardown belongs in `afterEach`, not the last line of a test body —
  // a failed assertion above it would skip `vi.useRealTimers()` and leak fake
  // timers into every later test in the file (an order-dependent green).
  afterEach(() => {
    vi.useRealTimers();
  });

  it('debounce-persists to the aggregate id once one exists', () => {
    vi.useFakeTimers();
    sessionBus.detail = detail();
    const generation = { id: 'gen-1', coverLetterText: '' } as AiGenerationRecord;
    const { result } = render({ latestGeneration: generation });

    act(() => result.current.editActiveOutput('hand-edited résumé'));
    vi.runAllTimers();

    expect(updateAiGenerationMutate).toHaveBeenCalledWith({
      id: 'gen-1',
      resumeText: 'hand-edited résumé',
    });
  });

  it('never calls updateAiGeneration without an aggregate id (session-only edit)', () => {
    vi.useFakeTimers();
    const { result } = render();
    act(() => result.current.editActiveOutput('edited text'));
    vi.runAllTimers();
    expect(updateAiGenerationMutate).not.toHaveBeenCalled();
  });

  // CR-2: unmounting inside the debounce window previously CLEARED the timer
  // without flushing — the pending write (and the local override that would
  // have re-surfaced it) both vanished with the component. Silent user data
  // loss: type a hand-edit, leave the tab (or the host remounts
  // `DocumentsTab`) before the debounce fires, and the edit never persists.
  it('flushes a pending edit on unmount instead of dropping it', () => {
    vi.useFakeTimers();
    sessionBus.detail = detail();
    const generation = { id: 'gen-1', coverLetterText: '' } as AiGenerationRecord;
    const { result, unmount } = render({ latestGeneration: generation });

    act(() => result.current.editActiveOutput('hand-edited résumé'));
    // Still inside the debounce window — nothing persisted YET, proving the
    // assertion below is about the unmount flush, not a race with the timer.
    expect(updateAiGenerationMutate).not.toHaveBeenCalled();

    unmount();

    expect(updateAiGenerationMutate).toHaveBeenCalledWith({
      id: 'gen-1',
      resumeText: 'hand-edited résumé',
    });
  });

  it('does not double-persist if the debounce timer somehow still fires after the unmount flush', () => {
    vi.useFakeTimers();
    sessionBus.detail = detail();
    const generation = { id: 'gen-1', coverLetterText: '' } as AiGenerationRecord;
    const { result, unmount } = render({ latestGeneration: generation });

    act(() => result.current.editActiveOutput('hand-edited résumé'));
    unmount();
    updateAiGenerationMutate.mockClear();

    vi.runAllTimers();

    expect(updateAiGenerationMutate).not.toHaveBeenCalled();
  });
});

describe('useTailorPipeline — the 4-step checklist position', () => {
  it('advances currentStep as the stage moves through the pipeline, never regressing on an unknown stage', () => {
    const { result, rerender } = render();
    expect(result.current.currentStep).toBe(0);

    sessionBus.stage = { stage: 'draft', phase: 'start' };
    rerender();
    expect(result.current.currentStep).toBe(1);

    sessionBus.stage = { stage: 'validate', phase: 'start' };
    rerender();
    expect(result.current.currentStep).toBe(2);

    // A stage name this build doesn't map — it holds rather than regressing.
    sessionBus.stage = { stage: 'a_future_stage', phase: 'start' };
    rerender();
    expect(result.current.currentStep).toBe(2);
  });
});

// The elapsed-timer fix (owner report: the clock reset to 0:00 on
// navigate-away-and-back): `GeneratingPanel` anchors on this field instead
// of its own mount time, so it must mirror the run RECORD's own backend
// timestamp — not anything reset by a remount of this hook.
describe('useTailorPipeline — runStartedAt (backend-anchored elapsed timer)', () => {
  it('is null before any run record has loaded', () => {
    const { result } = render();
    expect(result.current.runStartedAt).toBeNull();
  });

  it("mirrors the run record's own startedAt once it loads", () => {
    sessionBus.detail = detail({ status: 'running', startedAt: 12_345 });
    const { result } = render();
    expect(result.current.runStartedAt).toBe(12_345);
  });

  // The actual owner-reported path: navigating away unmounts the whole flow
  // (`ApplicationDetailPage` only renders `TailorFlow` while the Documents tab
  // is active) and a fresh mount reconnects via `initialRunId`/`initialJobId`.
  // A brand-new hook instance must still read the ORIGINAL start time off the
  // reconnected run record, not restart it.
  it('survives an unmount/remount ("navigate away and back") unchanged', () => {
    sessionBus.detail = detail({ status: 'running', startedAt: 12_345 });
    const first = render();
    expect(first.result.current.runStartedAt).toBe(12_345);
    first.unmount();

    const second = render({ initialRunId: 'run-1', initialJobId: 'job-1' });
    expect(second.result.current.runStartedAt).toBe(12_345);
  });

  // Defensive: not reachable with today's `now_ms()`-populated
  // `pipeline_runs.started_at` column, but `?? null` alone only guards
  // null/undefined — a `0` (or negative) value would otherwise become the
  // anchor and render an absurd/negative "N total" caption. `null` here is
  // what lets `GeneratingPanel`'s own `runStartedAt ?? mountFallback` recover
  // instead.
  it('falls back to null (not 0) for a non-positive startedAt', () => {
    sessionBus.detail = detail({ status: 'running', startedAt: 0 });
    const { result } = render();
    expect(result.current.runStartedAt).toBeNull();
  });
});

describe('useTailorPipeline — the section-fix / fabrication-review bundle', () => {
  it('is undefined until a run detail exists', () => {
    const { result } = render();
    expect(result.current.pipelineReview).toBeUndefined();
  });

  it('wires onFixSection to regenerateSection keyed on the run id', () => {
    sessionBus.detail = detail({ runId: 'run-7' });
    const { result } = render();
    expect(result.current.pipelineReview).toBeDefined();

    result.current.pipelineReview?.onFixSection?.('skills', 'be more specific');
    expect(regenerateMutate).toHaveBeenCalledWith({
      runId: 'run-7',
      sectionKey: 'skills',
      note: 'be more specific',
    });
  });

  it('wires onResolveFabrication to resolveFabrication keyed on the run id', () => {
    sessionBus.detail = detail({ runId: 'run-7' });
    const { result } = render();

    result.current.pipelineReview?.onResolveFabrication?.('code#0', 'keep');
    expect(resolveFabricationMutate).toHaveBeenCalledWith({
      runId: 'run-7',
      issueKey: 'code#0',
      decision: 'keep',
    });
  });
});

describe('useTailorPipeline — persisted-run notification', () => {
  it('calls onRunStarted once both ids are known', () => {
    const onRunStarted = vi.fn();
    sessionBus.runId = 'run-1';
    sessionBus.jobId = 'job-1';
    render({ onRunStarted });
    expect(onRunStarted).toHaveBeenCalledWith({ runId: 'run-1', jobId: 'job-1' });
  });

  it('never calls onRunStarted while only one id is known', () => {
    const onRunStarted = vi.fn();
    sessionBus.runId = 'run-1';
    sessionBus.jobId = null;
    render({ onRunStarted });
    expect(onRunStarted).not.toHaveBeenCalled();
  });

  // F1 regression: on the real DocumentsTab/TailorFlow wiring, `onRunStarted`
  // writes a Zustand slice, which ALWAYS returns a new object — re-rendering
  // the host, which passes a brand-new arrow back in. The prior effect listed
  // `onRunStarted` as a dependency with no already-persisted guard, so this
  // reproduced "Maximum update depth exceeded" immediately after a run
  // started. A test with a stable `vi.fn()` (the two tests above) cannot
  // catch this — it must pass a FRESH arrow every render, exactly like the
  // real host does.
  it('does not loop when onRunStarted is a fresh arrow every render (F1)', () => {
    const persisted: { runId: string; jobId: string }[] = [];
    sessionBus.runId = 'run-1';
    sessionBus.jobId = 'job-1';

    const { rerender } = renderHook(
      (props: { onRunStarted: (ids: { runId: string; jobId: string }) => void }) =>
        useTailorPipeline({ ...PARAMS, ...props }),
      {
        wrapper,
        initialProps: { onRunStarted: (ids) => persisted.push(ids) },
      }
    );

    // 20 re-renders, each passing a NEW closure — the exact shape that broke
    // (TailorFlow's inline `onRunStarted: (ids) => { persistence.setRun(...) }`).
    // If the guard regresses, this either throws React's max-update-depth
    // error or the callback fires 20 times instead of once.
    for (let i = 0; i < 20; i++) {
      rerender({ onRunStarted: (ids) => persisted.push(ids) });
    }

    expect(persisted).toEqual([{ runId: 'run-1', jobId: 'job-1' }]);
  });

  it('persists again once a NEW run id replaces the old one (guard is keyed, not one-shot)', () => {
    const onRunStarted = vi.fn();
    sessionBus.runId = 'run-1';
    sessionBus.jobId = 'job-1';
    const { rerender } = render({ onRunStarted });
    expect(onRunStarted).toHaveBeenCalledTimes(1);

    sessionBus.runId = 'run-2';
    sessionBus.jobId = 'job-2';
    rerender();

    expect(onRunStarted).toHaveBeenCalledTimes(2);
    expect(onRunStarted).toHaveBeenLastCalledWith({ runId: 'run-2', jobId: 'job-2' });
  });
});

describe('useTailorPipeline — cancel', () => {
  it('forwards to the session', () => {
    const { result } = render();
    result.current.cancel();
    expect(sessionBus.cancel).toHaveBeenCalledTimes(1);
  });
});

describe('useTailorPipeline — openClaimsTotal counts BOTH slots (H5)', () => {
  const minimalReport = {
    ok: true,
    issues: [],
    metrics: {
      keywordCoverage: null,
      topRequirementHits: null,
      duplicateRatio: 0,
      rolesSource: 0,
      rolesOutput: 0,
    },
  };

  it('sums unresolved fabrications from resume AND coverLetter, not just the active tab', () => {
    sessionBus.detail = detail({
      resumeText: 'Résumé text mentions FOO-EVIDENCE right here.',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 0,
        resume: {
          report: minimalReport,
          sourceTextHash: 0,
          fabrications: [{ issueKey: 'a#0', code: 'a', evidence: 'FOO-EVIDENCE' }],
        },
        coverLetter: {
          report: minimalReport,
          sourceTextHash: 0,
          fabrications: [{ issueKey: 'b#0', code: 'b', evidence: 'BAR-EVIDENCE' }],
        },
      },
    });
    const generation = {
      id: 'gen-1',
      coverLetterText: 'Cover letter mentions BAR-EVIDENCE right here.',
    } as AiGenerationRecord;

    const { result } = render({ latestGeneration: generation });

    // `activeOut` defaults to 'resume' — the OLD, buggy single-slot count
    // would report 1 here (only the resume's own fabrication).
    expect(result.current.activeOut).toBe('resume');
    expect(result.current.openClaimsTotal).toBe(2);
  });

  it('is 0 when neither slot has an unresolved fabrication', () => {
    sessionBus.detail = detail({
      resumeText: 'Clean résumé text.',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 0,
        resume: { report: minimalReport, sourceTextHash: 0 },
      },
    });
    const { result } = render();
    expect(result.current.openClaimsTotal).toBe(0);
  });
});

// The bug: `exportAs` used to pass the literal `undefined` in the `locale`
// position of both `exportPDF`/`exportDOCX`, which the Rust exporter resolves
// to market "intl" — silently dropping market-specific conventions (e.g. DIN
// 5008 for a German letter). `resolveMarket` is now computed once from the
// hook's own `targetLanguage` (detected from `jobDesc`) and threaded through.
// Real (unmocked) `detectLanguage` fixtures below are lifted verbatim from
// `packages/shared/src/language-detection.test.ts` — known-reliable inputs.
describe('useTailorPipeline — export market (DIN 5008 / locale-drop regression)', () => {
  const GERMAN_JOB_AD =
    'Erfahrener Softwareentwickler mit fundierten Kenntnissen in der Entwicklung skalierbarer Webanwendungen und verteilter Backend-Systeme für große Unternehmen.';
  const ENGLISH_JOB_AD =
    'Experienced software engineer with a strong background in building scalable web applications and distributed backend systems for large organisations.';

  function exportLocaleArg(mockFn: typeof exportPDF | typeof exportDOCX): unknown {
    const call = vi.mocked(mockFn).mock.calls.at(-1);
    return call?.[6];
  }

  it('sends the German market (not undefined, not "intl") for a German job ad — PDF', async () => {
    sessionBus.detail = detail({ resumeText: 'RESUME TEXT' });
    const { result } = render({ jobDesc: GERMAN_JOB_AD });

    await act(async () => {
      await result.current.exportAs('pdf');
    });

    expect(exportLocaleArg(exportPDF)).toBe('de');
  });

  it('sends the German market for a German job ad — DOCX', async () => {
    sessionBus.detail = detail({ resumeText: 'RESUME TEXT' });
    const { result } = render({ jobDesc: GERMAN_JOB_AD });

    await act(async () => {
      await result.current.exportAs('docx');
    });

    expect(exportLocaleArg(exportDOCX)).toBe('de');
  });

  it('sends the English/international market for an English job ad — PDF', async () => {
    sessionBus.detail = detail({ resumeText: 'RESUME TEXT' });
    const { result } = render({ jobDesc: ENGLISH_JOB_AD });

    await act(async () => {
      await result.current.exportAs('pdf');
    });

    expect(exportLocaleArg(exportPDF)).toBe('intl');
  });

  it('never leaves the locale argument undefined, regardless of language', async () => {
    sessionBus.detail = detail({ resumeText: 'RESUME TEXT' });
    const { result } = render({ jobDesc: ENGLISH_JOB_AD });

    await act(async () => {
      await result.current.exportAs('docx');
    });

    expect(exportLocaleArg(exportDOCX)).not.toBeUndefined();
  });

  // The live preview (GenerationOutput → PdfPreview) reads this SAME value off
  // the hook's return, not the export call args — without it exposed here the
  // preview silently renders under market "intl" while the export renders under
  // "de" (a German posting shows an English salutation on screen but a German
  // one in the downloaded file). Asserting the exposed value directly, not just
  // the export call, is what would fail if a future edit dropped it from the
  // return object.
  it('exposes the resolved market on the hook return (not just the export call)', () => {
    sessionBus.detail = detail({ resumeText: 'RESUME TEXT' });
    const { result } = render({ jobDesc: GERMAN_JOB_AD });

    expect(result.current.market).toBe('de');
  });

  // `jobLocation` is the found job's free-text location (e.g. "New York, NY,
  // US") — previously never read, so an ENGLISH posting always fell through
  // to `LANGUAGE_TO_MARKET.en === 'intl'` (A4) even for a US applicant.
  it('a US-located English posting resolves market "us" (US Letter), not "intl"', () => {
    sessionBus.detail = detail({ resumeText: 'RESUME TEXT' });
    const { result } = render({
      jobDesc: ENGLISH_JOB_AD,
      jobLocation: 'New York, NY, US',
    });

    expect(result.current.market).toBe('us');
  });

  it('an unlocated English posting still falls back to "intl"', () => {
    sessionBus.detail = detail({ resumeText: 'RESUME TEXT' });
    const { result } = render({ jobDesc: ENGLISH_JOB_AD, jobLocation: undefined });

    expect(result.current.market).toBe('intl');
  });
});

describe('useTailorPipeline — stageLabel fallback (L: no raw snake_case leak)', () => {
  it('falls back to the translated coarse state for a stage name this build does not have copy for', () => {
    sessionBus.state = 'drafting';
    sessionBus.stage = {
      stage: 'a_future_stage_this_build_predates',
      phase: 'start',
    };
    const { result } = render();
    // Never the raw wire name — falls back to pipeline.state.drafting's translation.
    expect(result.current.stageLabel).not.toBe('a_future_stage_this_build_predates');
    expect(result.current.stageLabel).toBe('pipeline.state.drafting');
  });
});

// Stage 6d — the fabricated `meta` stub silently dropped the "top requirement
// hits" metric from a Re-check (`use-quality-recheck.ts` sends
// `meta.topRequirements` verbatim) and short-circuited the answers
// assistant's own metadata extraction (`useApplicationAnswers.ts` treats any
// non-null `meta` as already detected). Reverting to the fabricated
// `topRequirements: []` makes this fail.
describe('useTailorPipeline — meta is seeded from the aggregate, not fabricated', () => {
  it("meta.topRequirements equals the record's list, not an empty array", () => {
    sessionBus.detail = detail({ resumeText: 'RESUME' });
    const generation = {
      id: 'gen-1',
      candidateName: 'Jane Doe',
      resumeLanguage: 'de',
      jobAdLanguage: 'en',
      mismatch: true,
      topRequirements: ['Kubernetes', 'Rust'],
      coverLetterText: '',
    } as AiGenerationRecord;

    const { result } = render({ latestGeneration: generation });

    expect(result.current.meta?.topRequirements).toEqual(['Kubernetes', 'Rust']);
    expect(result.current.meta?.candidateName).toBe('Jane Doe');
    expect(result.current.meta?.resumeLanguage).toBe('de');
    expect(result.current.meta?.jobAdLanguage).toBe('en');
    expect(result.current.meta?.mismatch).toBe(true);
  });

  it('falls back to the derived defaults when no aggregate exists', () => {
    sessionBus.detail = detail({ resumeText: 'RESUME' });
    const { result } = render();

    expect(result.current.meta?.topRequirements).toEqual([]);
    expect(result.current.meta?.candidateName).toBe('');
    expect(result.current.meta?.mismatch).toBe(false);
  });
});

// `resolveTargetLanguage` — the pure precedence chain `useTailorPipeline`'s
// `targetLanguage` memo wraps. Tested directly (no hook, no session mock) per
// the plan's "extract into a pure exported helper" note; the hook-level
// tests below cover the WIRING (the memo's output actually reaching
// `session.start`/`meta`), not the precedence logic itself.
describe('resolveTargetLanguage — precedence chain (Defect A/B fix)', () => {
  const GERMAN_JOB_AD =
    'Erfahrener Softwareentwickler mit fundierten Kenntnissen in der Entwicklung skalierbarer Webanwendungen und verteilter Backend-Systeme für große Unternehmen.';

  it('prefers the persisted targetLanguage — the field the staged pipeline actually writes — over everything else', () => {
    const generation = { targetLanguage: 'de', jobAdLanguage: 'en' } as AiGenerationRecord;
    expect(resolveTargetLanguage(generation, '')).toEqual({ language: 'de', confident: true });
  });

  // The English-lock regression test (Defect B): a résumé written in English
  // must NEVER pin the target the moment the pipeline hasn't confidently
  // written one yet — only the job ad's own language may. Mutation: read
  // `latestGeneration.resumeLanguage` back into the chain (tier 1 or 2) →
  // this goes red (`'en'` instead of `'de'`).
  it('ignores the source résumé language entirely, even when present on the record', () => {
    const generation = {
      resumeLanguage: 'English',
      targetLanguage: '',
      jobAdLanguage: '',
    } as AiGenerationRecord;
    expect(resolveTargetLanguage(generation, GERMAN_JOB_AD)).toEqual({
      language: 'de',
      confident: true,
    });
  });

  // The SAME persisted field carries two shapes: `extractMetadata` (the
  // AIGeneratePage flow) writes a display NAME like "German", every other
  // writer an ISO code, and `save_application` merges both into one record.
  // Preferring "German" verbatim is WORSE than the bug this chain fixes —
  // Rust truncates it to "ge", which matches no language arm, so the
  // language checks go dark for that document.
  it('normalizes a persisted language NAME to its ISO code before preferring it', () => {
    const generation = { targetLanguage: 'German', jobAdLanguage: '' } as AiGenerationRecord;
    expect(resolveTargetLanguage(generation, '')).toEqual({ language: 'de', confident: true });
  });

  // Each tier is validated INDEPENDENTLY: an invalid-but-present targetLanguage
  // must not short-circuit a perfectly good jobAdLanguage one rung down.
  it('falls through an invalid targetLanguage to a valid jobAdLanguage', () => {
    const generation = {
      targetLanguage: 'not-a-language',
      jobAdLanguage: 'de',
    } as AiGenerationRecord;
    expect(resolveTargetLanguage(generation, '')).toEqual({ language: 'de', confident: true });
  });

  it('falls back to jobAdLanguage when targetLanguage is empty', () => {
    const generation = { targetLanguage: '', jobAdLanguage: 'de' } as AiGenerationRecord;
    expect(resolveTargetLanguage(generation, '')).toEqual({ language: 'de', confident: true });
  });

  it('detects the language from the job ad when there is no previous generation (first run)', () => {
    expect(resolveTargetLanguage(undefined, GERMAN_JOB_AD)).toEqual({
      language: 'de',
      confident: true,
    });
  });

  // The negative case the owner explicitly asked to be pinned: when nothing
  // is confident, the chain still returns a usable code (generation must
  // proceed) — but flags it `confident: false` so the caller keeps it off
  // the wire. Mutation: return `confident: true` unconditionally → red.
  it('marks the last-resort English fallback as NOT confident, unlike a real detection', () => {
    expect(resolveTargetLanguage(undefined, '')).toEqual({ language: 'en', confident: false });
  });
});

describe('useTailorPipeline — targetLanguage precedence is wired end to end', () => {
  const GERMAN_JOB_AD =
    'Erfahrener Softwareentwickler mit fundierten Kenntnissen in der Entwicklung skalierbarer Webanwendungen und verteilter Backend-Systeme für große Unternehmen.';

  it('resolves targetLanguage from the previous generation even when jobDesc is empty (the failing regenerate condition)', async () => {
    sessionBus.detail = detail({ resumeText: 'RESUME' });
    const generation = {
      id: 'gen-1',
      targetLanguage: 'de',
      jobAdLanguage: 'en',
      coverLetterText: '',
    } as AiGenerationRecord;

    const { result } = render({ jobDesc: '', latestGeneration: generation });

    expect(result.current.meta?.targetLanguage).toBe('de');

    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(sessionBus.start).toHaveBeenCalledWith(
      expect.objectContaining({ targetLanguage: 'de' })
    );
  });

  it('detects targetLanguage from the job ad when there is no previous generation (first run)', async () => {
    const { result } = render({ jobDesc: GERMAN_JOB_AD, latestGeneration: undefined });

    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(sessionBus.start).toHaveBeenCalledWith(
      expect.objectContaining({ targetLanguage: 'de' })
    );
  });

  // The negative case: a guessed language must be neither PERSISTED nor
  // PREFERRED (owner decision). This is the "neither persisted" half —
  // `resolveTargetLanguage`'s "not confident" test above pins the "neither
  // preferred" half (a future run's tier 1 can only prefer what actually
  // reached the wire). Mutation: send `targetLanguage` (the resolved 'en')
  // instead of `wireTargetLanguage` in `start()` → this goes red.
  it('never sends a guessed language for persistence — the wire targetLanguage is empty when nothing was confident', async () => {
    const { result } = render({ jobDesc: '', latestGeneration: undefined });

    await act(async () => {
      await result.current.start({ resume: 'r', outputType: 'resume', researchCompany: false });
    });
    expect(sessionBus.start).toHaveBeenCalledWith(expect.objectContaining({ targetLanguage: '' }));
  });
});

// Stage 6d — a run stopped before the `validate` stage (cancel, or a deadline
// stop at a stage boundary) persists `session.detail.report === null`, which
// the OLD `session.detail ? session.detail.report : …` ternary took as a
// final answer and discarded the aggregate's perfectly good report. Reverting
// the `??` fallback to that ternary makes this fail.
describe('useTailorPipeline — quality report survives a terminal run with no live report', () => {
  const minimalReport = {
    ok: true,
    issues: [],
    metrics: {
      keywordCoverage: null,
      topRequirementHits: 3,
      duplicateRatio: 0,
      rolesSource: 0,
      rolesOutput: 0,
    },
  };

  it('falls back to the persisted report when a terminal detail.report is null', () => {
    sessionBus.detail = detail({ status: 'cancelled', report: null });
    const persisted = {
      schemaVersion: 2,
      pipeline: 'quality',
      generatedAt: 0,
      resume: { report: minimalReport, sourceTextHash: 0 },
    };
    const generation = {
      id: 'gen-1',
      coverLetterText: '',
      qualityReport: JSON.stringify(persisted),
    } as AiGenerationRecord;

    const { result } = render({ latestGeneration: generation });

    expect(result.current.report).not.toBeNull();
    expect(result.current.report?.resume?.report.metrics.topRequirementHits).toBe(3);
  });

  it('still prefers the live report when both exist', () => {
    sessionBus.detail = detail({
      status: 'completed',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 0,
        resume: { report: { ...minimalReport, ok: false }, sourceTextHash: 1 },
      },
    });
    const generation = {
      id: 'gen-1',
      coverLetterText: '',
      qualityReport: JSON.stringify({
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 0,
        resume: { report: minimalReport, sourceTextHash: 0 },
      }),
    } as AiGenerationRecord;

    const { result } = render({ latestGeneration: generation });

    expect(result.current.report?.resume?.sourceTextHash).toBe(1);
  });
});

// Stage 6e — a hard failure via the umbrella `job.failed` path (a full queue,
// no configured provider, a deleted résumé, …) sends `ERROR` straight to the
// session machine with NO run record ever written (`detail` stays `null`).
// The old effect gated on `session.detail?.status`, which never exists here,
// so the aggregate (and Autopilot's score) never refreshed. Reverting to that
// gate makes this fail.
describe('useTailorPipeline — a hard failure with no run record still invalidates', () => {
  it('invalidates aiGenerations AND autopilot once the session reaches ERROR with detail still null', () => {
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    const { rerender } = render();
    invalidateSpy.mockClear();

    sessionBus.state = 'error';
    sessionBus.detail = null;
    rerender();

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: keys.aiGenerations.all });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: keys.autopilot.all });
  });

  it('does not invalidate while still busy or idle', () => {
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    render();
    expect(invalidateSpy).not.toHaveBeenCalled();

    sessionBus.state = 'drafting';
    const { rerender } = render();
    rerender();
    expect(invalidateSpy).not.toHaveBeenCalled();
  });
});
