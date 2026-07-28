/**
 * ApplicationsPage — grouped rendering tests (Gap 4)
 *
 * Strategy:
 *  - `useApplications` is mocked at the module level so no IPC / QueryClient
 *    / AppClientProvider tree is needed.
 *  - `useSessionStore` (Zustand) is used directly: its initial state has no
 *    collapsed sections and an empty filter, which is what we want.
 *  - `motion/react` is replaced with plain fragments so animation code never
 *    runs.
 *  - `@ajh/translations` returns keys as-is.
 *  - `TrackJobModal` is stubbed to avoid its own provider requirements.
 *  - `ApplicationRow` is stubbed to a deterministic data-testid so section
 *    membership assertions are cheap and don't pull in that component's deps.
 *  - the `/applications/` index route `useSearch` + `useNavigate` are mocked so
 *    the page renders without a RouterProvider; `useSearch` returns `{}` (no `?highlight`),
 *    so the flash path is inert and these grouped-rendering tests are unaffected.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';

import type { Application } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

import { useSessionStore } from '@/store/session-store';

import { ApplicationsPage } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── motion/react ──────────────────────────────────────────────────────────────

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

// ── Router — render standalone (no RouterProvider) ────────────────────────────

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
}));

vi.mock('@/routes/applications.index', () => ({
  Route: { useSearch: () => ({}) },
}));

// ── Service hook — controlled mock ────────────────────────────────────────────

const mockUseApplications = vi.fn();

vi.mock('@/services/use-applications', () => ({
  useApplications: () => mockUseApplications(),
  // The page owns the optional-note prompt (see ApplicationsPage.notes.test.tsx);
  // inert here — these tests never change a stage.
  useSetApplicationStatus: () => ({ mutate: vi.fn(), isPending: false }),
}));

// ── ApplicationRow stub — renders a deterministic marker per application ──────

vi.mock('@/features/applications/components/ApplicationRow', () => ({
  ApplicationRow: ({
    application,
    showStageTag,
  }: {
    application: Application;
    showStageTag?: boolean;
  }) => (
    <div
      data-testid={TEST_IDS.applications.row}
      data-appid={application.id}
      data-status={application.status}
      data-stagetag={showStageTag ? 'true' : 'false'}
    >
      {application.title}
    </div>
  ),
}));

// ── TrackJobModal stub ────────────────────────────────────────────────────────

vi.mock('@/features/applications/components/TrackJobModal', () => ({
  TrackJobModal: () => <div data-testid={TEST_IDS.applications.trackJobModal} />,
}));

// ── PageShell stub — render children directly ─────────────────────────────────

// `actions` is rendered too — the header hosts the sort control the tests drive.
vi.mock('@/components/layout/PageShell', () => ({
  PageShell: ({ children, actions }: { children: React.ReactNode; actions?: React.ReactNode }) => (
    <div data-testid={TEST_IDS.layout.pageShell}>
      {actions}
      {children}
    </div>
  ),
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeApp(overrides: Partial<Application>): Application {
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

const APPS_MULTI_STAGE: Application[] = [
  makeApp({ id: 'a1', status: 'applied', title: 'Applied Role', company: 'Alpha' }),
  makeApp({ id: 'a2', status: 'applied', title: 'Applied Role 2', company: 'AlphaB' }),
  makeApp({ id: 'a3', status: 'interviewing', title: 'Interviewing Role', company: 'Beta' }),
  makeApp({ id: 'a4', status: 'saved', title: 'Saved Role', company: 'Gamma' }),
];

// ── Store reset ───────────────────────────────────────────────────────────────

beforeEach(() => {
  useSessionStore.setState((s) => ({
    applications: {
      ...s.applications,
      collapsedSections: [],
      filter: '',
      stageGroup: null,
      sort: 'updated',
    },
  }));
  mockUseApplications.mockReset();
});

/** Row ids in DOM order — the assertion surface for the sort tests. */
/** Stage-section headers only — the strip cards now share their labels. */
const sectionHeaders = (match: (name: string) => boolean) =>
  screen
    // `queryAll`, not `getAll`: "no section header matched" is a legitimate
    // expected outcome (flat aggregated group), not a test-setup failure.
    .queryAllByRole('button', { name: (n) => match(n) })
    .filter((el) => el.getAttribute('data-testid') !== TEST_IDS.applications.pipelineCard);

/** One pipeline-strip card by stage-group id. */
const pipelineCard = (group: string) =>
  screen
    .getAllByTestId(TEST_IDS.applications.pipelineCard)
    .find((c) => c.getAttribute('data-group') === group) as HTMLElement;

const rowIds = () =>
  screen.getAllByTestId(TEST_IDS.applications.row).map((r) => r.getAttribute('data-appid'));

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ApplicationsPage — grouped rendering', () => {
  it('renders one section per non-empty stage in APPLICATION_STAGES order', () => {
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });

    render(<ApplicationsPage />);

    // Three stages present: saved, applied, interviewing.
    // The section header button's accessible name includes the count badge so
    // it reads e.g. "applications.stages.saved 1" — match by substring.
    const stageKeys = [
      'applications.stages.saved',
      'applications.stages.applied',
      'applications.stages.interviewing',
    ];
    const headers = sectionHeaders((name) => stageKeys.some((key) => name.includes(key)));
    expect(headers).toHaveLength(3);

    // Verify APPLICATION_STAGES order: saved comes before applied comes before interviewing.
    const headerTexts = headers.map((h) => h.textContent ?? '');
    const savedIdx = headerTexts.findIndex((t) => t.includes('saved'));
    const appliedIdx = headerTexts.findIndex((t) => t.includes('applied'));
    const interviewingIdx = headerTexts.findIndex((t) => t.includes('interviewing'));
    expect(savedIdx).toBeLessThan(appliedIdx);
    expect(appliedIdx).toBeLessThan(interviewingIdx);
  });

  it('renders the correct count badge per section', () => {
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });

    render(<ApplicationsPage />);

    // Two ApplicationRow stubs for 'applied'.
    const appliedRows = screen
      .getAllByTestId(TEST_IDS.applications.row)
      .filter((el) => el.getAttribute('data-status') === 'applied');
    expect(appliedRows).toHaveLength(2);

    // One row for 'interviewing'.
    const interviewingRows = screen
      .getAllByTestId(TEST_IDS.applications.row)
      .filter((el) => el.getAttribute('data-status') === 'interviewing');
    expect(interviewingRows).toHaveLength(1);

    // One row for 'saved'.
    const savedRows = screen
      .getAllByTestId(TEST_IDS.applications.row)
      .filter((el) => el.getAttribute('data-status') === 'saved');
    expect(savedRows).toHaveLength(1);
  });

  it('renders EmptyState when the list is empty', () => {
    mockUseApplications.mockReturnValue({
      data: [],
      isLoading: false,
      isError: false,
    });

    render(<ApplicationsPage />);

    // The empty-list EmptyState uses the 'applications.empty' key as its title.
    expect(screen.getByText('applications.empty')).toBeInTheDocument();
    // No section headers rendered.
    expect(screen.queryByTestId(TEST_IDS.applications.row)).not.toBeInTheDocument();
  });

  it('does not render empty-stage sections (stages with zero apps are hidden)', () => {
    mockUseApplications.mockReturnValue({
      data: [makeApp({ id: 'x1', status: 'offer', title: 'Offer Role' })],
      isLoading: false,
      isError: false,
    });

    render(<ApplicationsPage />);

    // Only 'offer' section header should appear (name includes the count badge).
    expect(sectionHeaders((n) => n.includes('applications.stages.offer'))).toHaveLength(1);
    // 'applied' section must NOT be rendered.
    expect(sectionHeaders((n) => n.includes('applications.stages.applied'))).toHaveLength(0);
  });

  it('renders loading skeletons while isLoading=true and no rows', () => {
    mockUseApplications.mockReturnValue({
      data: undefined,
      isLoading: true,
      isError: false,
    });

    const { container } = render(<ApplicationsPage />);

    // Negative: no app rows and no empty-state.
    expect(screen.queryByTestId(TEST_IDS.applications.row)).not.toBeInTheDocument();
    expect(screen.queryByText('applications.empty')).not.toBeInTheDocument();

    // Positive: RowSkeleton renders elements with the animate-skeleton class.
    // ApplicationsPage renders 3 <RowSkeleton /> components, each of which
    // contains at least 2 inner Skeleton divs with class `animate-skeleton`.
    const skeletonShimmer = container.querySelectorAll('.animate-skeleton');
    expect(skeletonShimmer.length).toBeGreaterThan(0);
  });

  it('renders ErrorState when isError=true', () => {
    mockUseApplications.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
    });

    render(<ApplicationsPage />);

    expect(screen.getByText('applications.errorTitle')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.applications.row)).not.toBeInTheDocument();
  });

  // Gap 7 (MEDIUM): filter — only matching rows render when session-store filter is set.
  it('renders only rows matching the session-store filter substring (company match)', () => {
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });

    // Set the filter to a company substring that matches only APPS_MULTI_STAGE[0] ('Alpha').
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, collapsedSections: [], filter: 'alpha' },
    }));

    render(<ApplicationsPage />);

    const rows = screen.getAllByTestId(TEST_IDS.applications.row);
    // APPS_MULTI_STAGE has company 'Alpha' (a1) and 'AlphaB' (a2) — both match 'alpha'.
    // 'Beta' (a3) and 'Gamma' (a4) must NOT appear.
    expect(rows).toHaveLength(2);
    const appIds = rows.map((r) => r.getAttribute('data-appid'));
    expect(appIds).toContain('a1');
    expect(appIds).toContain('a2');
    expect(appIds).not.toContain('a3');
    expect(appIds).not.toContain('a4');
  });

  it('renders the noResults empty state when filter matches nothing', () => {
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });

    useSessionStore.setState((s) => ({
      applications: { ...s.applications, collapsedSections: [], filter: 'zzz-no-match' },
    }));

    render(<ApplicationsPage />);

    // noResults empty state is shown when allApps has rows but sections is empty.
    expect(screen.getByText('applications.noResults')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.applications.row)).not.toBeInTheDocument();
  });

  // An unexplained empty list right after clicking a stage card reads as data loss.
  it('names the active filters in the empty state and offers a way out', () => {
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, stageGroup: 'offer', filter: 'zzz' },
    }));

    render(<ApplicationsPage />);

    expect(screen.getByText('applications.noResultsBoth')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'applications.showAll' }));

    const { applications } = useSessionStore.getState();
    expect(applications.stageGroup).toBeNull();
    expect(applications.filter).toBe('');
  });

  it('explains a stage-only and a query-only empty result differently', () => {
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, stageGroup: 'offer', filter: '' },
    }));
    const { unmount } = render(<ApplicationsPage />);
    expect(screen.getByText('applications.noResultsStage')).toBeInTheDocument();
    unmount();

    useSessionStore.setState((s) => ({
      applications: { ...s.applications, stageGroup: null, filter: 'zzz' },
    }));
    render(<ApplicationsPage />);
    expect(screen.getByText('applications.noResultsQuery')).toBeInTheDocument();
  });

  // The strip's footprint must be held during load or the rows jump ~150px.
  it('reserves the strip footprint while loading', () => {
    mockUseApplications.mockReturnValue({ data: undefined, isLoading: true, isError: false });
    const { container } = render(<ApplicationsPage />);

    expect(container.querySelectorAll('.surface-card')).toHaveLength(6);
  });
});

// ── Pipeline strip — stage-group filtering ────────────────────────────────────

const APPS_CLOSED: Application[] = [
  makeApp({ id: 'c1', status: 'accepted', title: 'Accepted Role' }),
  makeApp({ id: 'c2', status: 'rejected', title: 'Rejected Role' }),
  makeApp({ id: 'c3', status: 'interviewing', title: 'Interviewing Role' }),
];

describe('ApplicationsPage — pipeline strip filter', () => {
  it('renders the strip only once there is at least one application', () => {
    mockUseApplications.mockReturnValue({ data: [], isLoading: false, isError: false });
    const { unmount } = render(<ApplicationsPage />);
    expect(screen.queryAllByTestId(TEST_IDS.applications.pipelineCard)).toHaveLength(0);
    unmount();

    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });
    render(<ApplicationsPage />);
    expect(screen.getAllByTestId(TEST_IDS.applications.pipelineCard)).toHaveLength(6);
  });

  it('clicking a card narrows the list to that stage group and marks it pressed', () => {
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });
    render(<ApplicationsPage />);

    fireEvent.click(pipelineCard('applied'));

    expect(useSessionStore.getState().applications.stageGroup).toBe('applied');
    // a1 + a2 are `applied`; the saved / interviewing rows are filtered out.
    expect(rowIds().sort()).toEqual(['a1', 'a2']);
    expect(pipelineCard('applied')).toHaveAttribute('aria-pressed', 'true');
  });

  it('clicking the active card again clears the filter', () => {
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, stageGroup: 'applied' },
    }));
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });
    render(<ApplicationsPage />);
    expect(rowIds()).toHaveLength(2);

    fireEvent.click(pipelineCard('applied'));

    expect(useSessionStore.getState().applications.stageGroup).toBeNull();
    expect(rowIds()).toHaveLength(4);
  });

  it('the Finished group renders ONE flat list, each row tagged with its own stage', () => {
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, stageGroup: 'closed' },
    }));
    mockUseApplications.mockReturnValue({ data: APPS_CLOSED, isLoading: false, isError: false });
    render(<ApplicationsPage />);

    expect(rowIds().sort()).toEqual(['c1', 'c2']);
    // The multi-stage group turns on the per-row stage Tag…
    screen
      .getAllByTestId(TEST_IDS.applications.row)
      .forEach((row) => expect(row.getAttribute('data-stagetag')).toBe('true'));
    // …and drops the per-stage headers, which would print the stage twice per row.
    expect(sectionHeaders((n) => n.includes('applications.stages.accepted'))).toHaveLength(0);
    expect(sectionHeaders((n) => n.includes('applications.stages.rejected'))).toHaveLength(0);
  });

  it('a single-stage group does NOT turn on the per-row stage Tag (the section header names it)', () => {
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, stageGroup: 'interviewing' },
    }));
    mockUseApplications.mockReturnValue({ data: APPS_CLOSED, isLoading: false, isError: false });
    render(<ApplicationsPage />);

    expect(rowIds()).toEqual(['c3']);
    expect(screen.getByTestId(TEST_IDS.applications.row).getAttribute('data-stagetag')).toBe(
      'false'
    );
  });

  it('an unknown persisted group degrades to the unfiltered list', () => {
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, stageGroup: 'not-a-group' },
    }));
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });
    render(<ApplicationsPage />);

    expect(rowIds()).toHaveLength(4);
  });

  it('the strip counts stay whole-list even while a filter narrows the rows', () => {
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, stageGroup: 'saved' },
    }));
    mockUseApplications.mockReturnValue({
      data: APPS_MULTI_STAGE,
      isLoading: false,
      isError: false,
    });
    render(<ApplicationsPage />);

    expect(pipelineCard('applied')).toHaveTextContent('2');
    expect(rowIds()).toEqual(['a4']);
  });
});

// ── Sort control ──────────────────────────────────────────────────────────────

const APPS_SAME_STAGE: Application[] = [
  makeApp({ id: 's1', status: 'applied', company: 'Zeta', updatedAt: 300, nextActionAt: 900 }),
  makeApp({ id: 's2', status: 'applied', company: 'Alpha', updatedAt: 100 }),
  makeApp({ id: 's3', status: 'applied', company: 'Mid', updatedAt: 200, nextActionAt: 500 }),
];

describe('ApplicationsPage — sort control', () => {
  it('defaults to `updated` (most recent first — the server order)', () => {
    mockUseApplications.mockReturnValue({
      data: APPS_SAME_STAGE,
      isLoading: false,
      isError: false,
    });
    render(<ApplicationsPage />);

    expect(rowIds()).toEqual(['s1', 's3', 's2']);
  });

  it('selecting "Company" re-orders the rows A→Z and persists the choice', async () => {
    mockUseApplications.mockReturnValue({
      data: APPS_SAME_STAGE,
      isLoading: false,
      isError: false,
    });
    render(<ApplicationsPage />);

    fireEvent.click(screen.getByRole('button', { name: /applications\.sort\.updated/i }));
    const listbox = await screen.findByRole('listbox');
    fireEvent.click(within(listbox).getByRole('option', { name: /applications\.sort\.company/i }));

    expect(useSessionStore.getState().applications.sort).toBe('company');
    expect(rowIds()).toEqual(['s2', 's3', 's1']);
  });

  it('`nextAction` puts the soonest reminder first and sinks reminder-less rows', () => {
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, sort: 'nextAction' },
    }));
    mockUseApplications.mockReturnValue({
      data: APPS_SAME_STAGE,
      isLoading: false,
      isError: false,
    });
    render(<ApplicationsPage />);

    expect(rowIds()).toEqual(['s3', 's1', 's2']);
  });
});
