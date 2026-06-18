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

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { Application } from '@ajh/shared';
import type { AiGenerationRecord } from '@ajh/shared/ipc';

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

vi.mock('@/routes/applications.$id', () => ({
  DETAIL_TABS: ['overview', 'timeline', 'brief', 'documents'] as const,
  Route: {
    useParams: () => ({ id: 'app-1' }),
    useSearch: () => ({ tab: mockTab }),
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
  };
  setApplicationApply: typeof mockSetApplicationApply;
} = {
  applicationApply: {
    applyWizardStep: 0,
    applyWizardForm: null,
    applyTemplateId: 'modern',
    applyAtsMode: false,
    applyForId: null,
  },
  setApplicationApply: mockSetApplicationApply,
};

vi.mock('@/store/session-store', () => ({
  useSessionStore: (selector?: (s: typeof mockSessionState) => unknown) =>
    selector ? selector(mockSessionState) : mockSessionState,
}));

// ── Hooks ─────────────────────────────────────────────────────────────────────

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => () => '',
}));

vi.mock('@/features/jobs/hooks/useDefaultResumeId', () => ({
  useDefaultResumeId: () => null,
}));

// ── TailorFlow stub — keep the heavy generation tree out of the render ────────

vi.mock('@/features/documents/components/TailorFlow', () => ({
  // Surface the injected seedGeneration id so we can assert DocumentsTab wires the
  // latest matching record (cold-entry hydration source).
  TailorFlow: ({ seedGeneration }: { seedGeneration?: { id: string } }) => (
    <div data-testid="tailor-flow" data-seedgenid={seedGeneration?.id ?? ''} />
  ),
}));

// ── GenerationCard stub ───────────────────────────────────────────────────────

vi.mock('@/features/documents/components/GenerationCard', () => ({
  GenerationCard: ({ gen }: { gen: AiGenerationRecord }) => (
    <div data-testid="generation-card" data-genid={gen.id} />
  ),
}));

// ── Service hooks — controlled mocks ─────────────────────────────────────────

const mockUseApplication = vi.fn();
const mockUseAiGenerations = vi.fn();
const mockUpdateApplicationMutate = vi.fn();
// Controlled so tests can assert `keepDocuments` on the delete path.
const mockRemoveMutateAsync = vi.fn().mockResolvedValue(undefined);
// JD-fetch (useImportJobUrl) — controllable so the recovery-panel fetch tests can
// drive onSuccess and toggle the isError messaging path.
const mockImportJobUrlMutate = vi.fn();
let mockImportJobUrlIsError = false;

vi.mock('@/services', () => ({
  useApplication: () => mockUseApplication(),
  useSetApplicationStatus: () => ({
    mutateAsync: vi.fn().mockResolvedValue(undefined),
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
 * Minimal generation fixture. Only `id` and `jobUrl` are read by the
 * component's filter; `GenerationCard` itself is stubbed, so the rest
 * of the AiGenerationRecord fields are cast placeholders.
 */
function makeGen(overrides: { id: string; jobUrl: string }): AiGenerationRecord {
  return {
    id: overrides.id,
    jobUrl: overrides.jobUrl,
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
  mockUseApplication.mockReset();
  mockUseAiGenerations.mockReset();
  mockUpdateApplicationMutate.mockClear();
  mockSetApplicationApply.mockClear();
  mockRemoveMutateAsync.mockClear();
  mockNavigate.mockClear();
  mockImportJobUrlMutate.mockReset();
  mockImportJobUrlIsError = false;
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
        makeGen({ id: 'gen-1', jobUrl: 'https://acme.com/job/1' }),
        makeGen({ id: 'gen-2', jobUrl: 'https://other.com/x' }),
      ],
    });

    render(<ApplicationDetailPage />);

    // The Documents tab is now a full-height host for TailorFlow (mirrors the
    // autopilot apply flow); the previously-saved generations list was removed.
    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
    expect(screen.getByTestId('tailor-flow')).toBeInTheDocument();
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
      data: [makeGen({ id: 'gen-x', jobUrl: 'https://different.com/job/99' })],
    });

    render(<ApplicationDetailPage />);

    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
    // TailorFlow still mounts so the user can generate inline.
    expect(screen.getByTestId('tailor-flow')).toBeInTheDocument();
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
        makeGen({ id: 'gen-1', jobUrl: 'https://acme.com/job/1' }),
        makeGen({ id: 'gen-2', jobUrl: 'https://other.com/x' }),
      ],
    });

    render(<ApplicationDetailPage />);

    // Only gen-1 matches the application's jobUrl → it seeds the flow.
    expect(screen.getByTestId('tailor-flow')).toHaveAttribute('data-seedgenid', 'gen-1');
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
      data: [makeGen({ id: 'gen-x', jobUrl: 'https://different.com/job/99' })],
    });

    render(<ApplicationDetailPage />);

    expect(screen.getByTestId('tailor-flow')).toHaveAttribute('data-seedgenid', '');
  });

  it('does NOT match generations when the application jobUrl is empty', () => {
    // The component's guard: `appUrl !== '' && g.jobUrl.trim() === appUrl`
    mockTab = 'documents';
    const app = makeApp({ jobUrl: '' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({
      data: [makeGen({ id: 'gen-z', jobUrl: '' })],
    });

    render(<ApplicationDetailPage />);

    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
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
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
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

  // ── contactEmail ─────────────────────────────────────────────────────────────

  it('blurring contactEmail with a CHANGED value calls mutate with the new contactEmail', () => {
    const app = makeApp({ id: 'app-ce-1', contactEmail: 'a@a.com' });
    renderLoadedApp(app);

    const field = screen.getByLabelText('applications.detail.contactEmailLabel');
    fireEvent.change(field, { target: { value: 'b@b.com' } });
    fireEvent.blur(field);

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
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
    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
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
    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
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
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/applications' });
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
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/applications' });
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
    expect(screen.getByText('jobUrlImport.notFound')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('applications.detail.jdPlaceholder')).toBeInTheDocument();
    expect(screen.queryByText('applications.detail.briefEmpty')).not.toBeInTheDocument();
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
    expect(screen.getByTestId('tailor-flow')).toBeInTheDocument();
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
