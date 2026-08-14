/**
 * ApplicationDetailPage — generation matching + not-found + loading + save-on-blur +
 * ?tab= routing + ActionMenu delete flows + Brief empty state + Documents tab toolbar
 *
 * Strategy (updated for the tabbed restructure):
 *  - All service hooks the page uses are mocked at module level via the
 *    `@/services` barrel so no IPC / QueryClient / AppClientProvider tree is
 *    needed. `useAiGenerations` is mocked separately (not in the barrel).
 *  - `Route.useParams` / `Route.useSearch` are mocked — `useSearch` returns the
 *    active tab so a test can target the Overview or Documents tab directly,
 *    rendering without a RouterProvider.
 *  - `useNavigate` is mocked to a hoisted spy so tab-navigation + delete-back
 *    assertions can assert the exact call arguments.
 *  - `useRemoveApplication` mutateAsync is a hoisted spy so delete-flow tests can
 *    assert `keepDocuments` values.
 *  - `useSessionStore` (no-selector form) returns the `applicationApply` slice +
 *    a `setApplicationApply` spy so the embedded DocumentsTab + reset effect run.
 *  - `TailorFlow` is stubbed to a deterministic marker so the heavy generation
 *    sub-tree (and its i18n-init transitive imports) never loads.
 *  - `GenerationCard` is stubbed to a marker so assertions are cheap.
 *  - `useFormatRelativeTime` is mocked to `() => () => ''` to keep it inert.
 *  - `@ajh/translations` returns keys as-is.
 *  - noUncheckedIndexedAccess: all array accesses are guarded.
 */

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { Application, StatusEvent } from '@ajh/shared';
import type { AiGenerationRecord } from '@ajh/shared/ipc';
import { TEST_IDS } from '@ajh/test-ids';

import type { TailorWizardState } from '@/features/documents/components/TailorFlow/lib/tailor-state';
import type { TemplateId } from '@/lib/generate';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── Router — render standalone (no RouterProvider) ────────────────────────────

// Hoist the spy so the new tab-routing + delete-navigation tests can assert on it.
const mockNavigate = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

// The active tab returned by `Route.useSearch()`. Defaults to overview; tests
// that target the Documents tab override it via `mockTab`.
let mockTab: 'overview' | 'timeline' | 'brief' | 'documents' = 'overview';
// The `?from=` origin returned by `Route.useSearch()`. Defaults to unset; the
// Back-navigation tests below override it per case.
let mockFrom: 'jobs' | 'autopilot' | 'applications' | undefined = undefined;

vi.mock('@/routes/applications.$id', () => ({
  DETAIL_TABS: ['overview', 'timeline', 'brief', 'documents'] as const,
  Route: {
    useParams: () => ({ id: 'app-1' }),
    useSearch: () => ({ tab: mockTab, from: mockFrom }),
  },
}));

// ── Session store — selector form ─────────────────────────────────────────────

const mockSetApplicationApply = vi.fn();

const mockSessionState: {
  applicationApply: {
    applyWizardStep: number;
    applyWizardForm: TailorWizardState | null;
    applyTemplateId: TemplateId;
    applyAtsMode: boolean;
    applyForId: string | null;
    applyRun: { forId: string; runId: string; jobId: string } | null;
  };
  setApplicationApply: typeof mockSetApplicationApply;
  // Only `lastAppliedId` is read by the component (the resetScroll gate); kept
  // minimal rather than mirroring the full autopilot slice.
  autopilot: { lastAppliedId: string | null };
} = {
  applicationApply: {
    applyWizardStep: 0,
    applyWizardForm: null,
    applyTemplateId: 'classic',
    applyAtsMode: false,
    applyForId: null,
    applyRun: null,
  },
  setApplicationApply: mockSetApplicationApply,
  autopilot: { lastAppliedId: null },
};

vi.mock('@/store/session-store', () => ({
  useSessionStore: (selector?: (s: typeof mockSessionState) => unknown) =>
    selector ? selector(mockSessionState) : mockSessionState,
}));

// ── Hooks ─────────────────────────────────────────────────────────────────────

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => () => '',
}));

// ── TailorFlow stub — keep the heavy generation tree out of the render ────────

// Capture the onJobDescChange callback so DocumentsTab debounce tests can
// simulate a job-ad edit without mounting real TailorFlow internals.
let capturedOnJobDescChange: ((text: string) => void) | undefined = undefined;

vi.mock('@/features/documents/components/TailorFlow', () => ({
  // Surface the injected seedGeneration id so we can assert DocumentsTab wires the
  // latest matching record (cold-entry hydration source). Also surface
  // `persistence.runId`/`runJobId` (F2 regression) — the self-describing-read
  // gate DocumentsTab applies to `applicationApply.applyRun` before ever
  // handing a reconnect target to TailorFlow.
  TailorFlow: ({
    seedGeneration,
    onJobDescChange,
    persistence,
  }: {
    seedGeneration?: { id: string };
    onJobDescChange?: (text: string) => void;
    persistence?: { runId: string | null; runJobId: string | null };
  }) => {
    capturedOnJobDescChange = onJobDescChange;
    return (
      <div
        data-testid={TEST_IDS.documents.tailorFlow}
        data-seedgenid={seedGeneration?.id ?? ''}
        data-runid={persistence?.runId ?? ''}
        data-runjobid={persistence?.runJobId ?? ''}
      />
    );
  },
}));

// ── GenerationCard stub ───────────────────────────────────────────────────────

vi.mock('@/features/documents/components/GenerationCard', () => ({
  GenerationCard: ({ gen }: { gen: AiGenerationRecord }) => (
    <div data-testid={TEST_IDS.documents.generationCard} data-genid={gen.id} />
  ),
}));

// ── Service hooks — controlled mocks ─────────────────────────────────────────

const mockUseApplication = vi.fn();
const mockUseAiGenerations = vi.fn();
const mockUpdateApplicationMutate = vi.fn();
/** `setStatus.mutate(vars, options)` — resolves successfully by default so the
 *  optional-note prompt opens (mirrors the row's mock). */
type StatusMutateOptions = { onSuccess?: () => void; onError?: () => void };
const mockSetStatusMutate = vi.fn((_vars: unknown, options?: StatusMutateOptions) => {
  options?.onSuccess?.();
});
// Controlled so tests can assert `keepDocuments` on the delete path.
const mockRemoveMutateAsync = vi.fn().mockResolvedValue(undefined);
// JD-fetch (useImportJobUrl) — controllable so the recovery-panel fetch tests can
// drive onSuccess and toggle the isError messaging path.
const mockImportJobUrlMutate = vi.fn();
let mockImportJobUrlIsError = false;
// JD-resolve (useResolveJobUrl) — controllable so BriefTab loading-state tests
// can simulate the in-flight window before the auto-resolve settles.
let mockResolveJobUrlIsFetching = false;

vi.mock('@/services', () => ({
  useApplication: () => mockUseApplication(),
  useSetApplicationStatus: () => ({
    mutate: mockSetStatusMutate,
    isPending: false,
  }),
  useUpdateApplication: () => ({
    mutate: mockUpdateApplicationMutate,
    isPending: false,
  }),
  useOpenExternal: () => ({
    mutate: vi.fn(),
  }),
  useRemoveApplication: () => ({
    mutateAsync: mockRemoveMutateAsync,
    isPending: false,
  }),
  useDocuments: () => ({ data: [], isLoading: false }),
  useDocumentText: () => ({ data: undefined, isLoading: false }),
  useImportJobUrl: () => ({
    mutate: mockImportJobUrlMutate,
    isPending: false,
    isError: mockImportJobUrlIsError,
  }),
  useResolveJobUrl: () => ({
    data: undefined,
    isFetching: mockResolveJobUrlIsFetching,
    isError: false,
    isFetched: false,
    refetch: vi.fn(),
  }),
}));

vi.mock('@/services/use-ai-generations', () => ({
  useAiGenerations: () => mockUseAiGenerations(),
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeApp(overrides: Partial<Application> = {}): Application {
  return {
    id: 'app-1',
    status: 'applied',
    createdAt: 1000,
    updatedAt: 1000,
    jobUrl: 'https://acme.com/job/1',
    board: 'linkedin',
    company: 'Acme',
    title: 'Engineer',
    candidate: 'Jane',
    answers: [],
    brief: '',
    notes: '',
    comp: '',
    jobDescription: '',
    jobSummary: '',
    contactName: '',
    contactEmail: '',
    ...overrides,
  };
}

/**
 * Minimal generation fixture. The component now joins docs by the `applicationId`
 * FK (not `jobUrl`); `GenerationCard` itself is stubbed, so the rest of the
 * AiGenerationRecord fields are cast placeholders.
 */
function makeGen(overrides: {
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

// ── Import component under test (after all mocks) ─────────────────────────────

import { ApplicationDetailPage } from './index';

// ── Reset between tests ───────────────────────────────────────────────────────

beforeEach(() => {
  mockTab = 'overview';
  mockFrom = undefined;
  mockSessionState.autopilot.lastAppliedId = null;
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
  mockImportJobUrlIsError = false;
  mockResolveJobUrlIsFetching = false;
  capturedOnJobDescChange = undefined;
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — generation matching (Documents tab)', () => {
  it('does NOT render a saved-generations list even when a generation matches (list removed)', () => {
    mockTab = 'documents';
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({
      data: [
        makeGen({ id: 'gen-1', jobUrl: 'https://acme.com/job/1', applicationId: 'app-1' }),
        makeGen({ id: 'gen-2', jobUrl: 'https://other.com/x', applicationId: 'app-2' }),
      ],
    });

    render(<ApplicationDetailPage />);

    // The Documents tab is now a full-height host for TailorFlow (mirrors the
    // autopilot apply flow); the previously-saved generations list was removed.
    expect(screen.queryByTestId(TEST_IDS.documents.generationCard)).not.toBeInTheDocument();
    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toBeInTheDocument();
  });

  it('shows no saved GenerationCard when no generation matches', () => {
    mockTab = 'documents';
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({
      data: [
        makeGen({
          id: 'gen-x',
          jobUrl: 'https://different.com/job/99',
          applicationId: 'app-other',
        }),
      ],
    });

    render(<ApplicationDetailPage />);

    expect(screen.queryByTestId(TEST_IDS.documents.generationCard)).not.toBeInTheDocument();
    // TailorFlow still mounts so the user can generate inline.
    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toBeInTheDocument();
  });

  it('passes the matching generation to TailorFlow as the seedGeneration (cold-entry source)', () => {
    mockTab = 'documents';
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({
      data: [
        makeGen({ id: 'gen-1', jobUrl: 'https://acme.com/job/1', applicationId: 'app-1' }),
        makeGen({ id: 'gen-2', jobUrl: 'https://other.com/x', applicationId: 'app-2' }),
      ],
    });

    render(<ApplicationDetailPage />);

    // Only gen-1 carries this application's FK → it seeds the flow.
    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toHaveAttribute(
      'data-seedgenid',
      'gen-1'
    );
  });

  it('passes no seedGeneration to TailorFlow when nothing matches', () => {
    mockTab = 'documents';
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({
      data: [
        makeGen({
          id: 'gen-x',
          jobUrl: 'https://different.com/job/99',
          applicationId: 'app-other',
        }),
      ],
    });

    render(<ApplicationDetailPage />);

    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toHaveAttribute('data-seedgenid', '');
  });

  it('does NOT match a generation whose applicationId differs from this application', () => {
    // Docs join by the `applicationId` FK, not by url — a generation linked to a
    // DIFFERENT Application (or unlinked) must not surface here, even if its url
    // happens to match.
    mockTab = 'documents';
    const app = makeApp({ id: 'app-1' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({
      data: [
        makeGen({ id: 'gen-z', jobUrl: 'https://acme.com/job/1', applicationId: 'app-other' }),
      ],
    });

    render(<ApplicationDetailPage />);

    expect(screen.queryByTestId(TEST_IDS.documents.generationCard)).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// F2 regression — a stale `applyRun` from another application must never
// reach TailorFlow as a reconnect target (cross-application run leak).
//
// Both apply entry points (`usePostingActions.handleTailor`,
// `AutopilotPage.handleApply`) set `applyForId` to the NEW application's id
// BEFORE navigating, with no compensating clear of the reconnect target — so
// the reset effect's `applyForId !== application.id` guard can already be
// FALSE by the time this application's DocumentsTab first renders (and even
// where it isn't, this tab is the wizard's DEFAULT tab, so it mounts in the
// SAME commit as the reset effect that would otherwise clear it — a classic
// `useState` lazy-initializer race). Exercised here directly at the render
// level: `applicationApply.applyRun.forId` disagreeing with the CURRENT
// application id, regardless of how that came to be, must read as "no run".
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — F2: applyRun is read only for the CURRENT application', () => {
  afterEach(() => {
    mockSessionState.applicationApply = {
      ...mockSessionState.applicationApply,
      applyRun: null,
    };
  });

  it('does NOT hand another application’s run id to TailorFlow (A→B leak)', () => {
    mockTab = 'documents';
    // Simulates the exact hazard: `applyForId` already matches THIS
    // application (set atomically by an apply entry point, or simply because
    // the reset effect already ran) while `applyRun` still points at a
    // DIFFERENT application's run — the shape a naive `applyRunId` field
    // could leak, and the self-describing `forId` gate must reject.
    mockSessionState.applicationApply = {
      ...mockSessionState.applicationApply,
      applyForId: 'app-1',
      applyRun: { forId: 'app-OTHER', runId: 'run-OTHER', jobId: 'job-OTHER' },
    };
    const app = makeApp({ id: 'app-1' });
    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<ApplicationDetailPage />);

    const flow = screen.getByTestId(TEST_IDS.documents.tailorFlow);
    expect(flow).toHaveAttribute('data-runid', '');
    expect(flow).toHaveAttribute('data-runjobid', '');
  });

  it('DOES hand this application’s own run id to TailorFlow (happy path)', () => {
    mockTab = 'documents';
    mockSessionState.applicationApply = {
      ...mockSessionState.applicationApply,
      applyForId: 'app-1',
      applyRun: { forId: 'app-1', runId: 'run-1', jobId: 'job-1' },
    };
    const app = makeApp({ id: 'app-1' });
    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<ApplicationDetailPage />);

    const flow = screen.getByTestId(TEST_IDS.documents.tailorFlow);
    expect(flow).toHaveAttribute('data-runid', 'run-1');
    expect(flow).toHaveAttribute('data-runjobid', 'job-1');
  });
});

describe('ApplicationDetailPage — save-on-blur (Overview tab)', () => {
  function renderLoadedApp(app: Application) {
    mockTab = 'overview';
    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });
    render(<ApplicationDetailPage />);
  }

  // ── notes ────────────────────────────────────────────────────────────────────

  it('blurring the notes field with an UNCHANGED value does NOT call the update mutation', () => {
    // The notes field is seeded from `application.notes`. Blurring without
    // editing leaves the buffer equal to the seed → no mutation.
    const app = makeApp({ notes: 'existing note' });
    renderLoadedApp(app);

    const notes = screen.getByLabelText('applications.detail.notesLabel');
    fireEvent.blur(notes);

    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();
  });

  it('blurring the notes field with a CHANGED value DOES call the update mutation', () => {
    const app = makeApp({ id: 'app-edit-1', notes: 'existing note' });
    renderLoadedApp(app);

    const notes = screen.getByLabelText('applications.detail.notesLabel');
    fireEvent.change(notes, { target: { value: 'new note text' } });
    fireEvent.blur(notes);

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-edit-1',
      notes: 'new note text',
    });
  });

  // ── contactName ──────────────────────────────────────────────────────────────

  it('blurring contactName with a CHANGED value calls mutate with the new contactName', () => {
    const app = makeApp({ id: 'app-cn-1', contactName: 'Alice' });
    renderLoadedApp(app);

    const field = screen.getByLabelText('applications.detail.contactNameLabel');
    fireEvent.change(field, { target: { value: 'Bob' } });
    fireEvent.blur(field);

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    // Second arg is the mutation's result callbacks (the write's `{ error }` is
    // surfaced inline) — assert only the payload.
    expect(mockUpdateApplicationMutate.mock.calls[0]?.[0]).toEqual({
      id: 'app-cn-1',
      contactName: 'Bob',
    });
  });

  it('blurring contactName with an UNCHANGED value does NOT call mutate', () => {
    const app = makeApp({ contactName: 'Alice' });
    renderLoadedApp(app);

    fireEvent.blur(screen.getByLabelText('applications.detail.contactNameLabel'));

    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();
  });

  it('does NOT wipe a contact saved elsewhere when the Overview field is blurred', () => {
    // Regression: the apply-by-email tab and Overview now edit the SAME
    // canonical contact pair, but the Overview buffers are seeded ONCE via
    // useState. Sequence: Overview mounts with an empty contact → the email tab
    // persists "Rita Recruiter" → the record refetches. If the loaded view is
    // not re-seeded, the Overview input still holds its stale '' and the next
    // blur there persists that empty string back, wiping what was just saved.
    // `useSyncedBuffer` re-seeds THIS field (and only this field) when its server
    // value changes — the fix that replaced the whole-view `updatedAt` remount.
    mockTab = 'overview';
    mockUseApplication.mockReturnValue({
      data: { application: makeApp({ id: 'app-wipe-1', contactName: '' }), events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });
    const { rerender } = render(<ApplicationDetailPage />);

    // The apply-by-email tab wrote the contact; the query refetches with a new
    // `updatedAt` (every persisted change bumps it server-side).
    mockUseApplication.mockReturnValue({
      data: {
        application: makeApp({
          id: 'app-wipe-1',
          contactName: 'Rita Recruiter',
          updatedAt: 2000,
        }),
        events: [],
      },
      isLoading: false,
      isError: false,
    });
    rerender(<ApplicationDetailPage />);

    const field = screen.getByLabelText('applications.detail.contactNameLabel');
    expect((field as HTMLInputElement).value).toBe('Rita Recruiter');

    fireEvent.blur(field);
    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();
  });

  // ── contactEmail ─────────────────────────────────────────────────────────────

  it('blurring contactEmail with a CHANGED value calls mutate with the new contactEmail', () => {
    const app = makeApp({ id: 'app-ce-1', contactEmail: 'a@a.com' });
    renderLoadedApp(app);

    const field = screen.getByLabelText('applications.detail.contactEmailLabel');
    fireEvent.change(field, { target: { value: 'b@b.com' } });
    fireEvent.blur(field);

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate.mock.calls[0]?.[0]).toEqual({
      id: 'app-ce-1',
      contactEmail: 'b@b.com',
    });
  });

  // ── comp ──────────────────────────────────────────────────────────────────────

  it('blurring comp with a CHANGED value calls mutate with the new comp', () => {
    const app = makeApp({ id: 'app-comp-1', comp: '80k' });
    renderLoadedApp(app);

    const field = screen.getByLabelText('applications.detail.compLabel');
    fireEvent.change(field, { target: { value: '90k' } });
    fireEvent.blur(field);

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({ id: 'app-comp-1', comp: '90k' });
  });

  it('blurring comp with an UNCHANGED value does NOT call mutate', () => {
    const app = makeApp({ comp: '80k' });
    renderLoadedApp(app);

    fireEvent.blur(screen.getByLabelText('applications.detail.compLabel'));

    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();
  });

  // ── nextActionAt — verifies toDateInputValue + fromDateInputValue via the rendered field ──

  it('blurring nextActionAt with a new date calls mutate with the epoch-ms number', () => {
    // Construct a known local date: 2024-03-15 → epoch via new Date(y, m-1, d).
    // Using local construction avoids TZ flake (matches fromDateInputValue exactly).
    const knownEpoch = new Date(2024, 2, 15).getTime(); // March 15 2024 local
    const app = makeApp({ id: 'app-date-1', nextActionAt: knownEpoch });
    renderLoadedApp(app);

    const field = screen.getByLabelText('applications.detail.nextActionLabel');
    // Verify toDateInputValue rendered the correct YYYY-MM-DD string.
    expect(field).toHaveValue('2024-03-15');

    // Change to a new date and blur → fromDateInputValue converts back to epoch.
    const newEpoch = new Date(2024, 5, 1).getTime(); // June 1 2024 local
    fireEvent.change(field, { target: { value: '2024-06-01' } });
    fireEvent.blur(field);

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-date-1',
      nextActionAt: newEpoch,
    });
  });

  it('blurring nextActionAt with an empty string calls mutate with null (cleared date)', () => {
    const knownEpoch = new Date(2024, 2, 15).getTime();
    const app = makeApp({ id: 'app-date-2', nextActionAt: knownEpoch });
    renderLoadedApp(app);

    const field = screen.getByLabelText('applications.detail.nextActionLabel');
    fireEvent.change(field, { target: { value: '' } });
    fireEvent.blur(field);

    // fromDateInputValue('') → null; null !== knownEpoch → mutate fires.
    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-date-2',
      nextActionAt: null,
    });
  });

  it('blurring nextActionAt with UNCHANGED value does NOT call mutate', () => {
    const knownEpoch = new Date(2024, 2, 15).getTime();
    const app = makeApp({ nextActionAt: knownEpoch });
    renderLoadedApp(app);

    // Blur without changing → fromDateInputValue('2024-03-15') === knownEpoch → no mutate.
    fireEvent.blur(screen.getByLabelText('applications.detail.nextActionLabel'));

    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();
  });

  it('blurring nextActionAt when both field and application have no date does NOT call mutate', () => {
    // nextActionAt is undefined on the app; toDateInputValue(undefined) → '';
    // fromDateInputValue('') → null; null === (undefined ?? null) → no mutate.
    const app = makeApp({ nextActionAt: undefined });
    renderLoadedApp(app);

    fireEvent.blur(screen.getByLabelText('applications.detail.nextActionLabel'));

    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();
  });
});

describe('ApplicationDetailPage — wizard reset on mount', () => {
  it('seeds applyForId for the active application when it differs from the slice', () => {
    const app = makeApp({ id: 'app-reset-1' });
    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<ApplicationDetailPage />);

    expect(mockSetApplicationApply).toHaveBeenCalledWith({
      applyForId: 'app-reset-1',
      applyWizardStep: 0,
      applyWizardForm: null,
      applySeedResume: null,
      applyMatchLevel: null,
      applyRun: null,
    });
  });

  it('does NOT call setApplicationApply when applyForId already matches the application id (idempotence guard)', () => {
    // GAP 5: the effect has an early-return guard:
    //   if (applicationApply.applyForId !== application.id) { ... }
    // When they already match the effect must no-op.
    const app = makeApp({ id: 'app-1' });

    // Override the session store mock so applyForId already equals the app id.
    // The top-level vi.mock factory returns mockSessionState; we mutate it here
    // and restore in beforeEach via mockSetApplicationApply.mockClear().
    const prevApplyForId = mockSessionState.applicationApply.applyForId;
    mockSessionState.applicationApply = {
      ...mockSessionState.applicationApply,
      applyForId: 'app-1',
    };

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<ApplicationDetailPage />);

    // The guard fires → setApplicationApply must NOT have been called.
    expect(mockSetApplicationApply).not.toHaveBeenCalled();

    // Restore so subsequent tests in this describe see the original state.
    mockSessionState.applicationApply = {
      ...mockSessionState.applicationApply,
      applyForId: prevApplyForId,
    };
  });
});

describe('ApplicationDetailPage — not-found / error state', () => {
  it('shows the not-found error state when application is null', () => {
    mockUseApplication.mockReturnValue({
      data: { application: null, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<ApplicationDetailPage />);

    expect(screen.getByText('applications.detail.notFound')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.documents.generationCard)).not.toBeInTheDocument();
  });

  it('shows the not-found error state when isError=true', () => {
    mockUseApplication.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<ApplicationDetailPage />);

    expect(screen.getByText('applications.detail.notFound')).toBeInTheDocument();
  });
});

describe('ApplicationDetailPage — loading state', () => {
  it('renders skeletons while loading and suppresses content', () => {
    mockUseApplication.mockReturnValue({
      data: undefined,
      isLoading: true,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    const { container } = render(<ApplicationDetailPage />);

    // No GenerationCard and no not-found text while loading.
    expect(screen.queryByTestId(TEST_IDS.documents.generationCard)).not.toBeInTheDocument();
    expect(screen.queryByText('applications.detail.notFound')).not.toBeInTheDocument();

    // RowSkeleton / CardSkeleton render real elements with animate-skeleton class.
    const skeletonShimmer = container.querySelectorAll('.animate-skeleton');
    expect(skeletonShimmer.length).toBeGreaterThan(0);
  });
});

// ── Shared helper for loaded-state renders ────────────────────────────────────

function renderLoaded(overrides: Partial<Application> = {}) {
  const app = makeApp(overrides);
  mockUseApplication.mockReturnValue({
    data: { application: app, events: [] },
    isLoading: false,
    isError: false,
  });
  mockUseAiGenerations.mockReturnValue({ data: [] });
  render(<ApplicationDetailPage />);
  return app;
}

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — ?tab= query-param behaviour
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — ?tab= behaviour', () => {
  it('defaults to the overview tab when no ?tab= param is present (mockTab coerced to overview)', () => {
    // Route.useSearch returns { tab: 'overview' } as set in mockTab default.
    // The overview tab renders the notes field.
    mockTab = 'overview';
    renderLoaded();
    // Tab button for overview has aria-selected=true.
    const overviewTab = screen.getByRole('tab', { name: /applications\.detail\.tabs\.overview/i });
    expect(overviewTab).toHaveAttribute('aria-selected', 'true');
  });

  it('invalid tab values coerce to overview via validateSearch (mockTab=undefined→overview)', () => {
    // The route's validateSearch maps unknown values to undefined; the component
    // then coerces undefined to 'overview' via `?? 'overview'`. We simulate by
    // setting mockTab to a value that's not in DETAIL_TABS and verify overview renders.
    // Since the mock directly drives tab, we set it to undefined via a cast.
    (mockTab as unknown) = undefined;
    renderLoaded();
    const overviewTab = screen.getByRole('tab', { name: /applications\.detail\.tabs\.overview/i });
    expect(overviewTab).toHaveAttribute('aria-selected', 'true');
  });

  it('validateSearch returns undefined for an unknown tab value', () => {
    // Unit-test the validateSearch logic inline (the route mock replaces the
    // real module, so we mirror the source logic directly).
    const DETAIL_TABS_INLINE = ['overview', 'timeline', 'brief', 'documents'] as const;
    const validateSearch = (s: Record<string, unknown>): { tab?: string } => ({
      tab: (DETAIL_TABS_INLINE as readonly string[]).includes(s.tab as string)
        ? (s.tab as string)
        : undefined,
    });

    expect(validateSearch({ tab: 'documents' })).toEqual({ tab: 'documents' });
    expect(validateSearch({ tab: 'overview' })).toEqual({ tab: 'overview' });
    expect(validateSearch({ tab: 'invalid' })).toEqual({ tab: undefined });
    expect(validateSearch({ tab: '' })).toEqual({ tab: undefined });
    expect(validateSearch({})).toEqual({ tab: undefined });
  });

  // `setTab` passes a FUNCTIONAL search updater `(prev) => ({ ...prev, tab })` so
  // the origin `from` param is preserved across tab switches. These tests assert
  // both the `replace: true` shape and that the updater merges `tab` onto `prev`
  // (incl. preserving an existing `from`).
  const lastSearchUpdater = () => {
    const call = mockNavigate.mock.calls.at(-1)?.[0] as {
      search: (prev: Record<string, unknown>) => Record<string, unknown>;
    };
    return call.search;
  };

  it('clicking a tab calls navigate with a functional search updater that sets tab + preserves from', async () => {
    mockTab = 'overview';
    const user = userEvent.setup();
    renderLoaded({ id: 'app-nav-1' });

    // Click the "timeline" tab.
    const timelineTab = screen.getByRole('tab', { name: /applications\.detail\.tabs\.timeline/i });
    await user.click(timelineTab);

    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({ search: expect.any(Function), replace: true })
    );
    expect(lastSearchUpdater()({ from: 'jobs' })).toEqual({ from: 'jobs', tab: 'timeline' });
  });

  it('clicking the brief tab sets tab: "brief" via the functional updater', async () => {
    mockTab = 'overview';
    const user = userEvent.setup();
    renderLoaded({ id: 'app-nav-2' });

    await user.click(screen.getByRole('tab', { name: /applications\.detail\.tabs\.brief/i }));

    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({ search: expect.any(Function), replace: true })
    );
    expect(lastSearchUpdater()({})).toEqual({ tab: 'brief' });
  });

  it('clicking the documents tab sets tab: "documents" via the functional updater', async () => {
    mockTab = 'overview';
    const user = userEvent.setup();
    renderLoaded({ id: 'app-nav-3' });

    await user.click(screen.getByRole('tab', { name: /applications\.detail\.tabs\.documents/i }));

    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({ search: expect.any(Function), replace: true })
    );
    expect(lastSearchUpdater()({ from: 'autopilot' })).toEqual({
      from: 'autopilot',
      tab: 'documents',
    });
  });

  it('renders the active tabpanel with the correct id for the current tab', () => {
    mockTab = 'timeline';
    renderLoaded();
    // The tabpanel id is `appdetail-panel-<tab>`.
    const panel = document.getElementById('appdetail-panel-timeline');
    expect(panel).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — Back navigation `resetScroll` seam
//
// `from === 'autopilot'` ALONE isn't proof a compensating scroll will run:
// `from` is a URL search param that survives native forward-navigation, while
// `lastAppliedId` is the one-shot session-store field Autopilot's own focus
// effect consumes on its next mount. The gate requires BOTH — `from ===
// 'autopilot'` AND a pending `lastAppliedId` — before skipping the router's
// scroll restoration. Mutation check: dropping either half of the `&&` (back
// to a bare `from !== 'autopilot'`, or a bare `navigate({ to: backTarget })`)
// turns the first two assertions below red.
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — Back navigation resetScroll seam', () => {
  it('returning from Autopilot WITH a pending focus opts out of scroll restoration', async () => {
    mockFrom = 'autopilot';
    mockSessionState.autopilot.lastAppliedId = 'ap-1';
    const user = userEvent.setup();
    renderLoaded({ id: 'app-back-autopilot' });

    await user.click(screen.getByText('applications.detail.backAutopilot'));

    expect(mockNavigate).toHaveBeenCalledWith({ to: '/autopilot', resetScroll: false });
  });

  it('returning from Autopilot WITHOUT a pending focus (e.g. forward-nav replay) keeps the default reset', async () => {
    mockFrom = 'autopilot';
    mockSessionState.autopilot.lastAppliedId = null;
    const user = userEvent.setup();
    renderLoaded({ id: 'app-back-autopilot-stale' });

    await user.click(screen.getByText('applications.detail.backAutopilot'));

    expect(mockNavigate).toHaveBeenCalledWith({ to: '/autopilot', resetScroll: true });
  });

  it('returning from Jobs keeps the default scroll reset (no compensating scroll effect there)', async () => {
    mockFrom = 'jobs';
    const user = userEvent.setup();
    renderLoaded({ id: 'app-back-jobs' });

    await user.click(screen.getByText('applications.detail.backJobs'));

    expect(mockNavigate).toHaveBeenCalledWith({ to: '/jobs', resetScroll: true });
  });

  it('returning with no origin (plain applications list) also keeps the default scroll reset', async () => {
    mockFrom = undefined;
    const user = userEvent.setup();
    renderLoaded({ id: 'app-back-default' });

    await user.click(screen.getByText('applications.detail.back'));

    expect(mockNavigate).toHaveBeenCalledWith({ to: '/applications', resetScroll: true });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — ActionMenu delete flows
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — ActionMenu delete flows', () => {
  async function openActionMenu(user: ReturnType<typeof userEvent.setup>) {
    const trigger = screen.getByRole('button', { name: /applications\.row\.actions/i });
    await user.click(trigger);
  }

  it('"delete (keep documents)" confirms with keepDocuments: true and navigates to /applications', async () => {
    mockTab = 'overview';
    const user = userEvent.setup();
    renderLoaded({ id: 'app-del-keep' });

    await openActionMenu(user);

    // Click the keep-docs menu item.
    const keepItem = screen.getByRole('menuitem', {
      name: /applications\.row\.deleteKeepDocs/i,
    });
    await user.click(keepItem);

    // ConfirmModal is now open — click the confirm button (text = key).
    const confirmBtn = screen.getByRole('button', {
      name: /applications\.delete\.confirm/i,
    });
    await user.click(confirmBtn);

    expect(mockRemoveMutateAsync).toHaveBeenCalledWith({
      id: 'app-del-keep',
      keepDocuments: true,
    });
    // After deletion navigates back to /applications.
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/applications', resetScroll: true });
  });

  it('"delete everything" confirms with keepDocuments: false and navigates to /applications', async () => {
    mockTab = 'overview';
    const user = userEvent.setup();
    renderLoaded({ id: 'app-del-all' });

    await openActionMenu(user);

    const deleteAllItem = screen.getByRole('menuitem', {
      name: /applications\.row\.deleteAll/i,
    });
    await user.click(deleteAllItem);

    const confirmBtn = screen.getByRole('button', {
      name: /applications\.delete\.confirm/i,
    });
    await user.click(confirmBtn);

    expect(mockRemoveMutateAsync).toHaveBeenCalledWith({
      id: 'app-del-all',
      keepDocuments: false,
    });
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/applications', resetScroll: true });
  });

  it('cancelling the confirm modal does NOT call remove mutate', async () => {
    mockTab = 'overview';
    const user = userEvent.setup();
    renderLoaded({ id: 'app-del-cancel' });

    await openActionMenu(user);
    await user.click(screen.getByRole('menuitem', { name: /applications\.row\.deleteKeepDocs/i }));

    // Close dialog without confirming — assert by the dialog disappearing rather
    // than targeting the close button's brittle aria-label.
    // Press Escape to dismiss (more robust than matching the exact close-icon label).
    await user.keyboard('{Escape}');

    // The dialog should be gone and remove must not have been called.
    expect(mockRemoveMutateAsync).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — Brief & answers tab
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — Brief & answers tab', () => {
  // The generic briefEmpty EmptyState early-return was removed: an empty
  // brief/answers/JD stub now renders the JD recovery panel (paste/fetch) instead
  // of vanishing — that panel IS the empty experience for a partial import.
  it('renders the JD recovery panel (paste TextArea + notFound prompt) for an empty stub', () => {
    mockTab = 'brief';
    renderLoaded({ brief: '', answers: [], jobDescription: '' });
    // The recovery prompt + paste field are shown; the old EmptyState is gone.
    // useResolveJobUrl mock defaults isFetching=false so jdLoading=false → panel visible.
    expect(screen.getByText('jobUrlImport.notFound')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('applications.detail.jdPlaceholder')).toBeInTheDocument();
    expect(screen.queryByText('applications.detail.briefEmpty')).not.toBeInTheDocument();
  });

  it('shows loading skeleton while auto-resolve is in-flight (jdLoading gate)', () => {
    mockTab = 'brief';
    mockResolveJobUrlIsFetching = true;
    renderLoaded({ brief: '', answers: [], jobDescription: '' });
    // While resolve is fetching, the not-found/paste recovery panel must NOT show —
    // that would be a false-empty flash before the description arrives.
    expect(screen.queryByText('jobUrlImport.notFound')).not.toBeInTheDocument();
    expect(
      screen.queryByPlaceholderText('applications.detail.jdPlaceholder')
    ).not.toBeInTheDocument();
    // The loading sentinel is present (aria-busy).
    const busyEl = screen.getByRole('status');
    expect(busyEl).toHaveAttribute('aria-busy', 'true');
  });

  it('typing into the paste TextArea updates it and enables the Save button', async () => {
    mockTab = 'brief';
    const user = userEvent.setup();
    renderLoaded({ brief: '', answers: [], jobDescription: '' });

    const paste = screen.getByPlaceholderText('applications.detail.jdPlaceholder');
    const save = screen.getByRole('button', { name: /applications\.detail\.jdSave/i });
    // Disabled while the draft is empty (fix 3a: value is the draft, so typing sticks).
    expect(save).toBeDisabled();

    await user.type(paste, 'Pasted JD text');

    expect(paste).toHaveValue('Pasted JD text');
    expect(save).toBeEnabled();
  });

  it('clicking Save persists the pasted JD via the update mutation', async () => {
    mockTab = 'brief';
    const user = userEvent.setup();
    renderLoaded({ id: 'app-jd-save', brief: '', answers: [], jobDescription: '' });

    await user.type(screen.getByPlaceholderText('applications.detail.jdPlaceholder'), 'New JD');
    await user.click(screen.getByRole('button', { name: /applications\.detail\.jdSave/i }));

    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith(
      { id: 'app-jd-save', jobDescription: 'New JD' },
      expect.any(Object)
    );
  });

  it('clicking Fetch resolves the description and persists it on success', async () => {
    mockTab = 'brief';
    const user = userEvent.setup();
    // Drive onSuccess synchronously with a posting carrying a description.
    mockImportJobUrlMutate.mockImplementation((_url, opts) => {
      opts?.onSuccess?.({ description: 'Fetched JD body' });
    });
    renderLoaded({ id: 'app-jd-fetch', brief: '', answers: [], jobDescription: '' });

    await user.click(screen.getByRole('button', { name: /applications\.detail\.jdFetch/i }));

    expect(mockImportJobUrlMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jd-fetch',
      jobDescription: 'Fetched JD body',
    });
  });

  it('shows the fetch-failed message when the JD fetch errors', () => {
    mockTab = 'brief';
    mockImportJobUrlIsError = true;
    renderLoaded({ brief: '', answers: [], jobDescription: '' });
    expect(screen.getByText('jobUrlImport.failed')).toBeInTheDocument();
  });

  it('renders the brief text and all answers when both are present', () => {
    mockTab = 'brief';
    renderLoaded({
      brief: 'Great company.',
      answers: [
        { id: 'qa1', question: 'Q1?', answer: 'A1.' },
        { id: 'qa2', question: 'Q2?', answer: 'A2.' },
      ],
    });
    expect(screen.getByText('Great company.')).toBeInTheDocument();
    expect(screen.getByText('Q1?')).toBeInTheDocument();
    expect(screen.getByText('Q2?')).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — Documents tab toolbar
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — Documents tab toolbar', () => {
  it('renders the TailorFlow stub on the Documents tab', () => {
    mockTab = 'documents';
    renderLoaded();
    expect(screen.getByTestId(TEST_IDS.documents.tailorFlow)).toBeInTheDocument();
  });

  it('does NOT render the Questions button when controller stage is not "done"', () => {
    // onController is never called because TailorFlow is stubbed (never fires).
    // controller stays null → Questions button is hidden.
    mockTab = 'documents';
    renderLoaded();
    // The referral button IS always visible; questions button only shows on done.
    expect(
      screen.queryByRole('button', { name: /autopilot\.apply\.questions\.title/i })
    ).not.toBeInTheDocument();
    // Referral button always present.
    expect(screen.getByRole('button', { name: /autopilot\.referral\.open/i })).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — DocumentsTab debounced jobDescription persist
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — DocumentsTab debounced jobDescription persist', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('passes onJobDescChange to TailorFlow on the documents tab', () => {
    mockTab = 'documents';
    renderLoaded({ id: 'app-jdc-1' });
    // The TailorFlow stub captures onJobDescChange — it must be a function.
    expect(typeof capturedOnJobDescChange).toBe('function');
  });

  it('calls updateApplication.mutate with jobDescription after the 600ms debounce', () => {
    mockTab = 'documents';
    renderLoaded({ id: 'app-jdc-2' });

    // Simulate a job-ad edit via the captured callback (mirrors what TailorFlow calls).
    act(() => {
      capturedOnJobDescChange?.('New pasted job ad text');
    });

    // Before the debounce fires: mutate must NOT have been called yet.
    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();

    // Advance past the 600ms debounce window.
    act(() => {
      vi.advanceTimersByTime(600);
    });

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jdc-2',
      jobDescription: 'New pasted job ad text',
    });
  });

  it('debounce resets on rapid edits — only the last value is persisted', () => {
    mockTab = 'documents';
    renderLoaded({ id: 'app-jdc-3' });

    act(() => {
      capturedOnJobDescChange?.('first');
    });
    act(() => {
      vi.advanceTimersByTime(300); // 300ms — debounce not yet fired
    });
    act(() => {
      capturedOnJobDescChange?.('second');
    });
    act(() => {
      vi.advanceTimersByTime(300); // another 300ms — debounce for 'first' would have fired, but was reset
    });

    // Debounce not yet complete for 'second' (only 300ms since last edit).
    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(300); // total 600ms since 'second' → fires
    });

    // Only one call, with the last value.
    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jdc-3',
      jobDescription: 'second',
    });
  });

  it('unmount flushes the pending edit immediately — edit is not lost when tab changes before 600ms', () => {
    // Regression guard for the blocking bug: user pastes job ad then switches tabs
    // within 600ms → DocumentsTab unmounts → pending write must still fire.
    mockTab = 'documents';
    const { unmount } = render(
      (() => {
        const app = makeApp({ id: 'app-jdc-flush' });
        mockUseApplication.mockReturnValue({
          data: { application: app, events: [] },
          isLoading: false,
          isError: false,
        });
        mockUseAiGenerations.mockReturnValue({ data: [] });
        return <ApplicationDetailPage />;
      })()
    );

    // Simulate the user editing the job ad text.
    act(() => {
      capturedOnJobDescChange?.('Pasted job ad — not yet saved');
    });

    // The debounce window has NOT elapsed yet — mutate must not have fired.
    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();

    // User switches tabs: DocumentsTab unmounts before the 600ms window.
    act(() => {
      unmount();
    });

    // The cleanup flush must have fired the mutate with the latest pending value.
    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jdc-flush',
      jobDescription: 'Pasted job ad — not yet saved',
    });
  });

  it('unmount flush uses the latest edit value when multiple rapid edits preceded the unmount', () => {
    // Edge case: user types 'first', then 'second', then unmounts — only 'second' should flush.
    mockTab = 'documents';
    const { unmount } = render(
      (() => {
        const app = makeApp({ id: 'app-jdc-flush-latest' });
        mockUseApplication.mockReturnValue({
          data: { application: app, events: [] },
          isLoading: false,
          isError: false,
        });
        mockUseAiGenerations.mockReturnValue({ data: [] });
        return <ApplicationDetailPage />;
      })()
    );

    act(() => {
      capturedOnJobDescChange?.('first draft');
    });
    act(() => {
      vi.advanceTimersByTime(200); // still within debounce
    });
    act(() => {
      capturedOnJobDescChange?.('second draft — the keeper');
    });

    // Unmount before the debounce completes.
    act(() => {
      unmount();
    });

    // Only one flush call with the most-recent pending value.
    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jdc-flush-latest',
      jobDescription: 'second draft — the keeper',
    });
  });

  it('wrong-application regression: flush targets the id captured at edit time, not the current application id', () => {
    // Regression guard for Fix 1 (CodeRabbit MAJOR):
    // User edits Application A's job ad. Before the 600ms fires, the component
    // instance is reused for Application B (same DocumentsTab instance, different
    // `application` prop). The flush must write to A's id, not B's.
    //
    // We simulate the A→B reuse by rendering with app A, calling onJobDescChange,
    // then re-rendering with app B (no unmount), then advancing the timer.
    mockTab = 'documents';
    const appA = makeApp({ id: 'app-a', jobDescription: '' });
    mockUseApplication.mockReturnValue({
      data: { application: appA, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    const { rerender } = render(<ApplicationDetailPage />);

    // Simulate A's job-ad edit (captures id='app-a' + text).
    act(() => {
      capturedOnJobDescChange?.("Application A's job ad");
    });

    // Reuse: re-render with application B before the debounce fires.
    const appB = makeApp({ id: 'app-b', jobDescription: '' });
    mockUseApplication.mockReturnValue({
      data: { application: appB, events: [] },
      isLoading: false,
      isError: false,
    });
    rerender(<ApplicationDetailPage />);

    // Advance past the debounce window — the pending flush from A must fire.
    act(() => {
      vi.advanceTimersByTime(600);
    });

    // Must have been called exactly once with A's id and A's text.
    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-a',
      jobDescription: "Application A's job ad",
    });
    // B's id must never appear in the mutate call.
    expect(mockUpdateApplicationMutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ id: 'app-b' })
    );
  });

  it('multiple complete debounce cycles each persist their own value independently', () => {
    // Verifies that after the first debounce fires and clears pendingJd, a second
    // edit round-trips correctly and is not lost or merged with the first.
    mockTab = 'documents';
    renderLoaded({ id: 'app-jdc-cycles' });

    // First edit cycle.
    act(() => {
      capturedOnJobDescChange?.('first value');
    });
    act(() => {
      vi.advanceTimersByTime(600);
    });

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenNthCalledWith(1, {
      id: 'app-jdc-cycles',
      jobDescription: 'first value',
    });

    // Second edit cycle — after the first timer has already fired.
    act(() => {
      capturedOnJobDescChange?.('second value');
    });
    act(() => {
      vi.advanceTimersByTime(600);
    });

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(2);
    expect(mockUpdateApplicationMutate).toHaveBeenNthCalledWith(2, {
      id: 'app-jdc-cycles',
      jobDescription: 'second value',
    });
  });
});

// ── Follow-up promotion — visible from every tab, tinted when overdue ─────────

describe('ApplicationDetailPage — follow-up promotion', () => {
  const NOW = 1_700_000_000_000;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });
  afterEach(() => vi.useRealTimers());

  const renderWith = (app: Application) => {
    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });
    render(<ApplicationDetailPage />);
  };

  it('shows an overdue chip in the header (persists across tabs) when the date has passed', () => {
    mockTab = 'documents';
    renderWith(makeApp({ nextActionAt: NOW - 86_400_000 }));

    expect(screen.getByText('applications.detail.followUpOverdue')).toBeInTheDocument();
    expect(screen.queryByText('applications.detail.followUpDue')).not.toBeInTheDocument();
  });

  it('shows an upcoming chip when the date is still in the future', () => {
    mockTab = 'documents';
    renderWith(makeApp({ nextActionAt: NOW + 86_400_000 }));

    expect(screen.getByText('applications.detail.followUpDue')).toBeInTheDocument();
    expect(screen.queryByText('applications.detail.followUpOverdue')).not.toBeInTheDocument();
  });

  it('shows no chip at all when no reminder is set', () => {
    mockTab = 'documents';
    renderWith(makeApp({ nextActionAt: undefined }));

    expect(screen.queryByText('applications.detail.followUpDue')).not.toBeInTheDocument();
    expect(screen.queryByText('applications.detail.followUpOverdue')).not.toBeInTheDocument();
  });

  it('leads the Overview sheet with its own Follow-up section carrying the date field', () => {
    mockTab = 'overview';
    renderWith(makeApp({ nextActionAt: NOW - 86_400_000 }));

    expect(screen.getByText('applications.detail.followUpSection')).toBeInTheDocument();
    // The field itself still lives under its established label (deep links + the
    // save-on-blur tests above depend on it).
    expect(screen.getByLabelText('applications.detail.nextActionLabel')).toBeInTheDocument();
    expect(screen.getByText('applications.detail.followUpOverdueHint')).toBeInTheDocument();
  });

  it('uses the neutral hint when there is no reminder yet', () => {
    mockTab = 'overview';
    renderWith(makeApp({ nextActionAt: undefined }));

    expect(screen.getByText('applications.detail.followUpNoneHint')).toBeInTheDocument();
    expect(screen.queryByText('applications.detail.followUpOverdueHint')).not.toBeInTheDocument();
  });
});

// ── Interaction log — the Timeline "Add note" entry point ────────────────────

describe('ApplicationDetailPage — timeline notes', () => {
  const renderTimeline = (events: StatusEvent[]) => {
    mockTab = 'timeline';
    mockUseApplication.mockReturnValue({
      data: { application: makeApp({ status: 'interviewing' }), events },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });
    render(<ApplicationDetailPage />);
  };

  it('"Add note" writes a SAME-status setStatus carrying the trimmed note', () => {
    renderTimeline([]);

    fireEvent.click(screen.getByRole('button', { name: 'applications.note.add' }));
    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: '  Sent a thank-you email  ' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.note.save' }));

    expect(mockSetStatusMutate).toHaveBeenCalledTimes(1);
    expect(mockSetStatusMutate.mock.calls[0]?.[0]).toEqual({
      id: 'app-1',
      status: 'interviewing',
      note: 'Sent a thank-you email',
    });
  });

  it('the Add-note prompt uses the plain copy, not the post-transition copy', () => {
    renderTimeline([]);

    fireEvent.click(screen.getByRole('button', { name: 'applications.note.add' }));

    expect(screen.getByText('applications.note.current')).toBeInTheDocument();
    expect(screen.queryByText('applications.note.afterChange')).not.toBeInTheDocument();
  });

  it('renders a same-status note event as ONE stage (never "X → X") with its note', () => {
    renderTimeline([
      {
        applicationId: 'app-1',
        fromStatus: 'interviewing',
        toStatus: 'interviewing',
        at: 1_700_000_000_000,
        note: 'Recruiter call booked',
      },
    ]);

    // Scope to the tab panel: the header Dropdown also renders the stage label.
    const panel = within(screen.getByRole('tabpanel'));
    expect(panel.getByText('Recruiter call booked')).toBeInTheDocument();
    // A real transition renders the "from" label too; a note event must not.
    expect(panel.getAllByText('applications.status.interviewing')).toHaveLength(1);
  });

  it('still renders a real transition as from → to', () => {
    renderTimeline([
      {
        applicationId: 'app-1',
        fromStatus: 'applied',
        toStatus: 'interviewing',
        at: 1_700_000_000_000,
        note: '',
      },
    ]);

    const panel = within(screen.getByRole('tabpanel'));
    expect(panel.getByText('applications.status.applied')).toBeInTheDocument();
    expect(panel.getByText('applications.status.interviewing')).toBeInTheDocument();
  });
});

// ── No remount on refetch — focus + uncommitted input survive a landing write ──
//
// The loaded view was briefly keyed by `${id}:${updatedAt}`, which fixed the
// stale-seed contact wipe by remounting on EVERY persisted write — destroying
// focus, discarding text typed while a sibling write was in flight, and tearing
// down the whole TailorFlow sub-tree. The buffers now re-seed per field instead.

describe('ApplicationDetailPage — refetch does not remount the loaded view', () => {
  const renderOverview = (app: Application) => {
    mockTab = 'overview';
    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });
    return render(<ApplicationDetailPage />);
  };

  it('an uncommitted sibling edit and its caret survive another field write landing', () => {
    const { rerender } = renderOverview(makeApp({ id: 'app-live-1', notes: '', comp: '' }));

    // The user edits Notes and blurs it (write in flight), then types into Comp
    // WITHOUT blurring, leaving the caret there.
    const notes = screen.getByLabelText('applications.detail.notesLabel');
    fireEvent.change(notes, { target: { value: 'called the recruiter' } });
    fireEvent.blur(notes);

    const comp = screen.getByLabelText('applications.detail.compLabel');
    comp.focus();
    fireEvent.change(comp, { target: { value: '90k' } });
    expect(document.activeElement).toBe(comp);

    // The notes write lands: the record refetches with the new notes AND a bumped
    // updatedAt. Comp is untouched server-side.
    mockUseApplication.mockReturnValue({
      data: {
        application: makeApp({
          id: 'app-live-1',
          notes: 'called the recruiter',
          comp: '',
          updatedAt: 9999,
        }),
        events: [],
      },
      isLoading: false,
      isError: false,
    });
    rerender(<ApplicationDetailPage />);

    // The uncommitted "90k" is still there…
    const compAfter = screen.getByLabelText<HTMLInputElement>('applications.detail.compLabel');
    expect(compAfter.value).toBe('90k');
    // …and so is the caret (no remount ⇒ the same node keeps focus).
    expect(document.activeElement).toBe(compAfter);
    // The committed field shows the server value.
    expect(screen.getByLabelText<HTMLTextAreaElement>('applications.detail.notesLabel').value).toBe(
      'called the recruiter'
    );
  });

  it('a bumped updatedAt alone does not reset an untouched buffer', () => {
    const { rerender } = renderOverview(makeApp({ id: 'app-live-2', comp: '' }));

    const comp = screen.getByLabelText('applications.detail.compLabel');
    fireEvent.change(comp, { target: { value: 'draft only' } });

    // An unrelated write (a status change / note) bumps the record.
    mockUseApplication.mockReturnValue({
      data: { application: makeApp({ id: 'app-live-2', comp: '', updatedAt: 4242 }), events: [] },
      isLoading: false,
      isError: false,
    });
    rerender(<ApplicationDetailPage />);

    expect(screen.getByLabelText<HTMLInputElement>('applications.detail.compLabel').value).toBe(
      'draft only'
    );
  });
});

// ── Status change — no-op guard + failure surface ─────────────────────────────

describe('ApplicationDetailPage — header stage Dropdown', () => {
  const renderHeader = (app: Application) => {
    mockTab = 'overview';
    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });
    render(<ApplicationDetailPage />);
  };

  /** Opens the header stage Dropdown and picks `option`. */
  const pickStage = async (current: string, option: string) => {
    fireEvent.click(
      screen.getByRole('button', { name: new RegExp(`applications\\.status\\.${current}`, 'i') })
    );
    const listbox = await screen.findByRole('listbox');
    fireEvent.click(
      within(listbox).getByRole('option', {
        name: new RegExp(`applications\\.status\\.${option}`, 'i'),
      })
    );
  };

  it('re-picking the CURRENT stage writes nothing and opens no note prompt', async () => {
    renderHeader(makeApp({ id: 'app-noop', status: 'applied' }));

    await pickStage('applied', 'applied');

    expect(mockSetStatusMutate).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('surfaces a localized error (and no note prompt) when the transition FAILS', async () => {
    mockSetStatusMutate.mockImplementation((_vars: unknown, options?: StatusMutateOptions) => {
      options?.onError?.();
    });
    renderHeader(makeApp({ id: 'app-fail', status: 'applied' }));

    await pickStage('applied', 'offer');

    expect(screen.getByRole('alert')).toHaveTextContent('applications.row.statusError');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('opens the note prompt after a PERSISTED transition', async () => {
    renderHeader(makeApp({ id: 'app-ok', status: 'applied' }));

    await pickStage('applied', 'offer');

    expect(screen.getByRole('dialog')).toHaveAccessibleName('applications.note.title');
  });
});

// ── Contact writes surface a rejected result (same field as ApplyByEmailTab) ───

describe('ApplicationDetailPage — contact write rejection', () => {
  const renderOverview = (app: Application) => {
    mockTab = 'overview';
    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });
    render(<ApplicationDetailPage />);
  };

  const rejectWith = (error?: string) =>
    mockUpdateApplicationMutate.mockImplementation(
      (_vars: unknown, options?: { onSuccess?: (data: { error?: string }) => void }) => {
        options?.onSuccess?.(error ? { error } : {});
      }
    );

  it('shows an alert when the backend rejects the contact email', () => {
    rejectWith('invalid email');
    renderOverview(makeApp({ id: 'app-ce', contactEmail: '' }));

    const field = screen.getByLabelText('applications.detail.contactEmailLabel');
    fireEvent.change(field, { target: { value: 'not-an-email' } });
    fireEvent.blur(field);

    expect(screen.getByRole('alert')).toHaveTextContent('applications.detail.email.emailInvalid');
  });

  it('shows an alert when the backend rejects the contact name', () => {
    rejectWith('rejected');
    renderOverview(makeApp({ id: 'app-cn', contactName: '' }));

    const field = screen.getByLabelText('applications.detail.contactNameLabel');
    fireEvent.change(field, { target: { value: 'Dana' } });
    fireEvent.blur(field);

    expect(screen.getByRole('alert')).toHaveTextContent('applications.detail.contactSaveError');
  });

  it('shows no alert when the write is accepted', () => {
    rejectWith(undefined);
    renderOverview(makeApp({ id: 'app-ok2', contactEmail: '' }));

    const field = screen.getByLabelText('applications.detail.contactEmailLabel');
    fireEvent.change(field, { target: { value: 'dana@acme.com' } });
    fireEvent.blur(field);

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
