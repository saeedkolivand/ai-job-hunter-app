import { createElement, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import type { AiGenerationRecord } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';

import type * as GenerateModule from '@/lib/generate';

import { useTailorPipeline } from './useTailorPipeline';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

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
    vi.useRealTimers();
  });

  it('never calls updateAiGeneration without an aggregate id (session-only edit)', () => {
    vi.useFakeTimers();
    const { result } = render();
    act(() => result.current.editActiveOutput('edited text'));
    vi.runAllTimers();
    expect(updateAiGenerationMutate).not.toHaveBeenCalled();
    vi.useRealTimers();
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

    // max-depth-only stage name — this build doesn't map it, so it holds.
    sessionBus.stage = { stage: 'sections', phase: 'start' };
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
});

describe('useTailorPipeline — cancel', () => {
  it('forwards to the session', () => {
    const { result } = render();
    result.current.cancel();
    expect(sessionBus.cancel).toHaveBeenCalledTimes(1);
  });
});
