import { createElement, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import type { AiGenerationRecord } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';

import type * as GenerateModule from '@/lib/generate';

import { useTailorPipeline } from './useTailorPipeline';

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

const wrapper = ({ children }: { children: ReactNode }) =>
  createElement(QueryClientProvider, { client: new QueryClient() }, children);

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
