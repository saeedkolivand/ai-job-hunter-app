/**
 * Shared mocks + fixtures for the ApplicationDetailPage suites.
 *
 * Strategy:
 *  - All service hooks the page uses are mocked at module level via the
 *    `@/services` barrel so no IPC / QueryClient / AppClientProvider tree is
 *    needed. `useAiGenerations` is mocked separately (not in the barrel).
 *  - `Route.useParams` / `Route.useSearch` are mocked — `useSearch` returns the
 *    active tab (`state.tab`) and origin (`state.from`), rendering without a
 *    RouterProvider. `useNavigate` is a hoisted spy.
 *  - `useSessionStore` returns the `applicationApply` slice + a
 *    `setApplicationApply` spy so the embedded DocumentsTab + reset effect run.
 *  - `TailorFlow` / `GenerationCard` are stubbed to deterministic markers so the
 *    heavy generation sub-tree never loads.
 *  - `@ajh/translations` returns keys as-is.
 *
 * `./test-render` registers the `vi.mock`s that read this state; a suite imports the
 * page from there. The mutable knobs live on `state` so a suite can flip them per test.
 */

import { type Mock, vi } from 'vitest';

import type { Application, StatusEvent } from '@ajh/shared';
import type { AiGenerationRecord } from '@ajh/shared/ipc';

import type { TailorWizardState } from '@/features/documents/components/TailorFlow/lib/tailor-state';
import type { TemplateId } from '@/lib/generate';

// ── Controllable state ────────────────────────────────────────────────────────

type Tab = 'overview' | 'timeline' | 'brief' | 'documents';

export const state: {
  /** The active tab returned by `Route.useSearch()`. */
  tab: Tab;
  /** The `?from=` origin returned by `Route.useSearch()`. */
  from: 'jobs' | 'autopilot' | 'applications' | undefined;
  importIsError: boolean;
  /** `useResolveJobUrl().isFetching` — simulates the in-flight auto-resolve. */
  resolveFetching: boolean;
  /** Saved-documents list + the default résumé's text (`seedResumeDocId` shapes). */
  docsData: { _id: string; name?: string; isDefault?: boolean }[];
  documentText: string | undefined;
  /** Captured from the TailorFlow stub so the debounce tests can simulate a job-ad edit. */
  capturedOnJobDescChange: ((text: string) => void) | undefined;
} = {
  tab: 'overview',
  from: undefined,
  importIsError: false,
  resolveFetching: false,
  docsData: [],
  documentText: undefined,
  capturedOnJobDescChange: undefined,
};

export const mockNotify: Record<
  'open' | 'success' | 'error' | 'info' | 'warning' | 'destroy',
  Mock
> = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};
export const mockNavigate: Mock = vi.fn();
export const mockSetApplicationApply: Mock = vi.fn();

export const mockSessionState: {
  applicationApply: {
    applyWizardStep: number;
    applyWizardForm: TailorWizardState | null;
    applyTemplateId: TemplateId;
    applyAtsMode: boolean;
    applyForId: string | null;
    applyRun: { forId: string; runId: string; jobId: string } | null;
    /** The autopilot one-shot seed — text only, never a saved-doc id. */
    applySeedResume: string | null;
  };
  setApplicationApply: typeof mockSetApplicationApply;
  // Only `lastAppliedId` is read by the component (the resetScroll gate).
  autopilot: { lastAppliedId: string | null };
} = {
  applicationApply: {
    applyWizardStep: 0,
    applyWizardForm: null,
    applyTemplateId: 'classic',
    applyAtsMode: false,
    applyForId: null,
    applyRun: null,
    applySeedResume: null,
  },
  setApplicationApply: mockSetApplicationApply,
  autopilot: { lastAppliedId: null },
};

export const mockUseApplication: Mock = vi.fn();
export const mockUseAiGenerations: Mock = vi.fn();
export const mockUpdateApplicationMutate: Mock = vi.fn();
/** `setStatus.mutate(vars, options)` — resolves successfully by default so the
 *  optional-note prompt opens (mirrors the row's mock). */
export type StatusMutateOptions = { onSuccess?: () => void; onError?: () => void };
export const mockSetStatusMutate = vi.fn((_vars: unknown, options?: StatusMutateOptions) => {
  options?.onSuccess?.();
});
// Controlled so tests can assert `keepDocuments` on the delete path.
export const mockRemoveMutateAsync: Mock = vi.fn().mockResolvedValue(undefined);
export const mockImportJobUrlMutate: Mock = vi.fn();
/** `acceptStatusEvent.mutate`/`rejectStatusEvent.mutate` — resolve successfully
 *  (an empty, error-free result) by default. `onSuccess` takes the real
 *  `{ error? }` payload — the production handler branches on `data.error`.
 *  `onSettled` clears the component's OWN per-eventId in-flight tracking (a
 *  single shared observer can't represent two concurrently-pending rows). */
export type StatusEventMutateOptions = {
  onSuccess?: (data: { error?: string }) => void;
  onError?: () => void;
  onSettled?: () => void;
};
const resolveStatusEvent = (_vars: unknown, options?: StatusEventMutateOptions) => {
  options?.onSuccess?.({});
  options?.onSettled?.();
};
export const mockAcceptStatusEventMutate = vi.fn(resolveStatusEvent);
export const mockRejectStatusEventMutate = vi.fn(resolveStatusEvent);

import { makeApplication as makeApp } from '@/features/applications/lib/test-fixtures';

export { makeApp };

// ── Fixtures ──────────────────────────────────────────────────────────────────

/**
 * Minimal generation fixture. The component joins docs by the `applicationId`
 * FK (not `jobUrl`); `GenerationCard` itself is stubbed, so the rest of the
 * AiGenerationRecord fields are placeholders.
 */
export function makeGen(overrides: {
  id: string;
  jobUrl: string;
  applicationId?: string;
}): AiGenerationRecord {
  return {
    id: overrides.id,
    jobUrl: overrides.jobUrl,
    applicationId: overrides.applicationId,
    createdAt: 0,
    candidateName: '',
    jobTitle: '',
    companyName: '',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    targetLanguage: 'en',
    mismatch: false,
    topRequirements: [],
    mode: 'standard',
    resumeText: '',
    coverLetterText: '',
    jobAd: '',
    board: '',
    applicationAnswers: [],
    companyBrief: '',
    interviewQuestions: [],
  };
}

/** Status-event fixture — defaults to an ordinary settled user transition.
 *  `eventId` defaults to 1; tests exercising TWO provisional rows at once
 *  must override it per row so each fixture has a distinct, assertable identity. */
export function makeEvent(overrides: Partial<StatusEvent> = {}): StatusEvent {
  return {
    eventId: 1,
    applicationId: 'app-1',
    fromStatus: 'applied',
    toStatus: 'interviewing',
    at: 1000,
    note: '',
    source: 'user',
    confirmed: true,
    ...overrides,
  };
}

// ── Setup helpers ─────────────────────────────────────────────────────────────

/** Point `useApplication` at a loaded record (or `null` for the not-found shape). */
export function mockApp(application: Application | null, events: StatusEvent[] = []) {
  mockUseApplication.mockReturnValue({
    data: { application, events },
    isLoading: false,
    isError: false,
  });
}

/** `mockApp` + the generations list (default: none). */
export function setLoaded(
  application: Application | null,
  events: StatusEvent[] = [],
  generations: AiGenerationRecord[] = []
) {
  mockApp(application, events);
  mockUseAiGenerations.mockReturnValue({ data: generations });
}

/** `beforeEach` body — resets every spy and knob to its default. */
export function resetMocks() {
  state.tab = 'overview';
  state.from = undefined;
  state.importIsError = false;
  state.resolveFetching = false;
  state.docsData = [];
  state.documentText = undefined;
  state.capturedOnJobDescChange = undefined;
  mockSessionState.autopilot.lastAppliedId = null;
  mockSessionState.applicationApply.applySeedResume = null;
  mockUseApplication.mockReset();
  mockUseAiGenerations.mockReset();
  // `mockReset` (not `mockClear`): the contact-rejection tests install an
  // implementation that would otherwise leak into every later test.
  mockUpdateApplicationMutate.mockReset();
  mockSetStatusMutate.mockClear();
  mockSetStatusMutate.mockImplementation((_vars: unknown, options?: StatusMutateOptions) => {
    options?.onSuccess?.();
  });
  mockSetApplicationApply.mockClear();
  mockRemoveMutateAsync.mockClear();
  mockNavigate.mockClear();
  mockImportJobUrlMutate.mockReset();
  for (const m of [mockAcceptStatusEventMutate, mockRejectStatusEventMutate]) {
    m.mockClear();
    m.mockImplementation(resolveStatusEvent);
  }
  mockNotify.success.mockClear();
  mockNotify.error.mockClear();
}
