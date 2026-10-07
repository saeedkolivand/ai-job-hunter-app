/**
 * Harness for the `useTailorPipeline` tests: the `vi.mock` calls (hoisted above this
 * module's imports), the controlled session bus, and render helpers.
 *
 * Tests import the hook ONLY from here (re-exported below) — never from
 * '../useTailorPipeline' directly — so the mocks are registered before the hook
 * loads regardless of import sorting.
 */
import { createElement, type ReactNode } from 'react';
import { expect, type Mock, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import type { AiGenerationRecord } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';

import type * as GenerateModule from '@/lib/generate';
import { exportDOCX as exportDOCXImpl, exportPDF as exportPDFImpl } from '@/lib/generate';

import {
  resolveTargetLanguage as resolveTargetLanguageImpl,
  useTailorPipeline,
} from '../useTailorPipeline';

// Plain bindings (not `export { x }` of an import — vite-node leaves those undefined here).
export const exportDOCX = exportDOCXImpl;
export const exportPDF = exportPDFImpl;
export const resolveTargetLanguage = resolveTargetLanguageImpl;
export const useTailorPipelineForTest = useTailorPipeline;

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

export const mockNotify: { error: Mock } = { error: vi.fn() };
vi.mock('@ajh/ui', () => ({ useNotification: () => mockNotify }));

// ── Session — the seam this hook wraps. Fully controlled by the test. ────────

export const sessionBus = {
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
  start: vi.fn() as Mock,
  cancel: vi.fn() as Mock,
  reset: vi.fn() as Mock,
};

vi.mock('@/hooks/use-resume-pipeline-session', () => ({
  useResumePipelineSession: () => ({ ...sessionBus }),
}));

// ── Sibling service hooks — stub, capture calls ───────────────────────────────

export const regenerateMutate: Mock = vi.fn();
export const resolveFabricationMutate: Mock = vi.fn();
export const updateAiGenerationMutate: Mock = vi.fn();

vi.mock('@/services/use-resume-pipeline', () => ({
  usePipelineRunsForJob: () => ({ data: [] }),
  useRegenerateSection: () => ({ mutate: regenerateMutate, isPending: false, error: null }),
  useResolveFabrication: () => ({
    mutate: resolveFabricationMutate,
    isPending: false,
    error: null,
  }),
}));

export const activeConfig = { effort: undefined as string | undefined };
vi.mock('@/services/use-ai-provider', () => ({ useGenerateConfig: () => activeConfig }));

vi.mock('@/services/use-ai-generations', () => ({
  useUpdateAiGeneration: () => ({ mutate: updateAiGenerationMutate }),
}));

// Records what the hook was ASKED for. `recheck` is derived from
// `onReportChange` here exactly as the real hook derives it ("No session writer
// means no way to show a result — hide the action", `use-quality-recheck.ts`),
// so a test can assert on the returned action instead of on this stub's own
// hardcoded value — which is what the previous `recheck: undefined` stub made
// impossible.
export const qualityRecheckArgs = {
  current: null as { onReportChange?: unknown } | null,
};
vi.mock('@/hooks/use-quality-recheck', () => ({
  useQualityRecheck: (args: { onReportChange?: unknown }) => {
    qualityRecheckArgs.current = args;
    return { recheck: args.onReportChange ? () => {} : undefined, rechecking: false };
  },
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
export const getQueryClient = () => queryClient;
export const wrapper = ({ children }: { children: ReactNode }) =>
  createElement(QueryClientProvider, { client: queryClient }, children);

export const PARAMS = {
  jobDesc: 'a very German-language job ad'.repeat(1), // language detection is best-effort; not asserted precisely
  sourceResume: 'my resume',
  jobUrl: 'https://acme.com/job/1',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  board: 'linkedin',
  canUse: true,
  hasDesc: true,
  // The wizard's own default (`buildTailorDefaults`), so every existing case
  // keeps describing a run that produces both documents.
  target: 'both' as const,
  templateId: 'classic' as const,
  atsMode: false,
};

export function render(overrides: Partial<Parameters<typeof useTailorPipeline>[0]> = {}) {
  return renderHook(() => useTailorPipeline({ ...PARAMS, ...overrides }), { wrapper });
}

export function detail(overrides: Partial<PipelineRunDetail> = {}): PipelineRunDetail {
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

/** A partial aggregate record typed as the full one (the hook only reads the fields a test sets). */
export const record = (r: Partial<AiGenerationRecord>) => r as AiGenerationRecord;

/** Fixtures — real (unmocked) `detectLanguage` inputs lifted verbatim from
 *  `packages/shared/src/language-detection.test.ts` (known-reliable). */
export const GERMAN_JOB_AD =
  'Erfahrener Softwareentwickler mit fundierten Kenntnissen in der Entwicklung skalierbarer Webanwendungen und verteilter Backend-Systeme für große Unternehmen.';
export const ENGLISH_JOB_AD =
  'Experienced software engineer with a strong background in building scalable web applications and distributed backend systems for large organisations.';

type StartValues = Parameters<ReturnType<typeof useTailorPipeline>['start']>[0];

/** Starts a run inside `act` (résumé-only unless `values` say otherwise). */
export const startResumeRun = (
  result: { current: ReturnType<typeof useTailorPipeline> },
  values: Partial<StartValues> = {}
) =>
  act(async () => {
    await result.current.start({
      resume: 'r',
      outputType: 'resume',
      researchCompany: false,
      ...values,
    });
  });

/** Renders with `overrides`, starts a run, asserts the wire request, returns the hook result. */
export async function expectStartRequest(
  overrides: Parameters<typeof render>[0],
  expected: Record<string, unknown>,
  values: Partial<StartValues> = {}
) {
  const { result } = render(overrides);
  await startResumeRun(result, values);
  expect(sessionBus.start).toHaveBeenCalledWith(expect.objectContaining(expected));
  return result;
}

/** Call in `beforeEach`. */
export function resetHarness() {
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
  activeConfig.effort = undefined;
  regenerateMutate.mockClear();
  resolveFabricationMutate.mockClear();
  updateAiGenerationMutate.mockClear();
  mockNotify.error.mockClear();
  vi.mocked(exportPDF).mockClear();
  vi.mocked(exportDOCX).mockClear();
}
