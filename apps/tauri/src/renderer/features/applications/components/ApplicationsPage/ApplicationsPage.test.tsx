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
import { render, screen } from '@testing-library/react';

import type { Application } from '@ajh/shared';

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
}));

// ── ApplicationRow stub — renders a deterministic marker per application ──────

vi.mock('@/features/applications/components/ApplicationRow', () => ({
  ApplicationRow: ({ application }: { application: Application }) => (
    <div data-testid="application-row" data-appid={application.id} data-status={application.status}>
      {application.title}
    </div>
  ),
}));

// ── TrackJobModal stub ────────────────────────────────────────────────────────

vi.mock('@/features/applications/components/TrackJobModal', () => ({
  TrackJobModal: () => <div data-testid="track-job-modal" />,
}));

// ── PageShell stub — render children directly ─────────────────────────────────

vi.mock('@/components/layout/PageShell', () => ({
  PageShell: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="page-shell">{children}</div>
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
    },
  }));
  mockUseApplications.mockReset();
});

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
    const sectionHeaders = screen.getAllByRole('button', {
      name: (name) => stageKeys.some((key) => name.includes(key)),
    });
    expect(sectionHeaders).toHaveLength(3);

    // Verify APPLICATION_STAGES order: saved comes before applied comes before interviewing.
    const headerTexts = sectionHeaders.map((h) => h.textContent ?? '');
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
      .getAllByTestId('application-row')
      .filter((el) => el.getAttribute('data-status') === 'applied');
    expect(appliedRows).toHaveLength(2);

    // One row for 'interviewing'.
    const interviewingRows = screen
      .getAllByTestId('application-row')
      .filter((el) => el.getAttribute('data-status') === 'interviewing');
    expect(interviewingRows).toHaveLength(1);

    // One row for 'saved'.
    const savedRows = screen
      .getAllByTestId('application-row')
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
    expect(screen.queryByTestId('application-row')).not.toBeInTheDocument();
  });

  it('does not render empty-stage sections (stages with zero apps are hidden)', () => {
    mockUseApplications.mockReturnValue({
      data: [makeApp({ id: 'x1', status: 'offer', title: 'Offer Role' })],
      isLoading: false,
      isError: false,
    });

    render(<ApplicationsPage />);

    // Only 'offer' section header should appear (name includes the count badge).
    expect(
      screen.getByRole('button', { name: (n) => n.includes('applications.stages.offer') })
    ).toBeInTheDocument();
    // 'applied' section must NOT be rendered.
    expect(
      screen.queryByRole('button', { name: (n) => n.includes('applications.stages.applied') })
    ).not.toBeInTheDocument();
  });

  it('renders loading skeletons while isLoading=true and no rows', () => {
    mockUseApplications.mockReturnValue({
      data: undefined,
      isLoading: true,
      isError: false,
    });

    const { container } = render(<ApplicationsPage />);

    // Negative: no app rows and no empty-state.
    expect(screen.queryByTestId('application-row')).not.toBeInTheDocument();
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
    expect(screen.queryByTestId('application-row')).not.toBeInTheDocument();
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

    const rows = screen.getAllByTestId('application-row');
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
    expect(screen.queryByTestId('application-row')).not.toBeInTheDocument();
  });
});
