/**
 * ApplicationsPage — ?highlight deep-link consumption (Priority 3)
 *
 * Strategy:
 *  - Isolated test file so the `useSearch` override for highlight doesn't affect
 *    the grouped-rendering tests in ApplicationsPage.test.tsx.
 *  - `useSearch` is backed by a mutable variable so we can set it per-test.
 *  - `useNavigate` is a controlled spy so we can assert the URL-clear call.
 *  - `ApplicationRow` stub emits `data-highlighted` when the `highlighted` prop
 *    is truthy, so assertions are cheap.
 *  - `useApplications` returns a fixture with one "applied" row whose id is the
 *    highlight target.
 *  - Fake timers control the 3.5 s flash-clear timeout.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';

import type { Application } from '@ajh/shared';

import { useSessionStore } from '@/store/session-store';

import { ApplicationsPage } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── Router — controlled per-test ──────────────────────────────────────────────

// `currentSearch` is mutated before each test so `useSearch` returns the right value.
let currentSearch: Record<string, string | undefined> = {};
const mockNavigate = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('@/routes/applications.index', () => ({
  Route: {
    useSearch: () => currentSearch,
  },
}));

// ── Service hook ──────────────────────────────────────────────────────────────

const mockUseApplications = vi.fn();

vi.mock('@/services/use-applications', () => ({
  useApplications: () => mockUseApplications(),
}));

// ── ApplicationRow stub — exposes `highlighted` as a data attribute ────────────

vi.mock('@/features/applications/components/ApplicationRow', () => ({
  ApplicationRow: ({
    application,
    highlighted,
  }: {
    application: Application;
    highlighted?: boolean;
  }) => (
    <div
      data-testid="application-row"
      data-appid={application.id}
      data-status={application.status}
      data-highlighted={highlighted ? 'true' : 'false'}
    >
      {application.title}
    </div>
  ),
}));

// ── Stubs for page dependencies ───────────────────────────────────────────────

vi.mock('@/features/applications/components/TrackJobModal', () => ({
  TrackJobModal: () => <div data-testid="track-job-modal" />,
}));

vi.mock('@/components/layout/PageShell', () => ({
  PageShell: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="page-shell">{children}</div>
  ),
}));

// ── Fixture ───────────────────────────────────────────────────────────────────

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

const TARGET_ID = 'highlight-target';
const OTHER_ID = 'other-app';

const APPS = [
  makeApp({ id: TARGET_ID, status: 'applied', title: 'Target Role', company: 'Acme' }),
  makeApp({ id: OTHER_ID, status: 'applied', title: 'Other Role', company: 'Beta' }),
];

// ── Store reset ───────────────────────────────────────────────────────────────

beforeEach(() => {
  useSessionStore.setState((s) => ({
    applications: { ...s.applications, collapsedSections: [], filter: '' },
  }));
  mockUseApplications.mockReset();
  mockNavigate.mockReset();
  currentSearch = {};
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ApplicationsPage — ?highlight consumption', () => {
  it('passes highlighted=true to the matching ApplicationRow when ?highlight is set', async () => {
    currentSearch = { highlight: TARGET_ID };
    mockUseApplications.mockReturnValue({ data: APPS, isLoading: false, isError: false });

    await act(async () => {
      render(<ApplicationsPage />);
    });

    const targetRow = screen
      .getAllByTestId('application-row')
      .find((el) => el.getAttribute('data-appid') === TARGET_ID);
    expect(targetRow).toBeDefined();
    expect(targetRow?.getAttribute('data-highlighted')).toBe('true');
  });

  it('does NOT pass highlighted=true to non-matching rows', async () => {
    currentSearch = { highlight: TARGET_ID };
    mockUseApplications.mockReturnValue({ data: APPS, isLoading: false, isError: false });

    await act(async () => {
      render(<ApplicationsPage />);
    });

    const otherRow = screen
      .getAllByTestId('application-row')
      .find((el) => el.getAttribute('data-appid') === OTHER_ID);
    expect(otherRow).toBeDefined();
    expect(otherRow?.getAttribute('data-highlighted')).toBe('false');
  });

  it('clears the ?highlight URL param via navigate({search:{},replace:true}) after consuming it', async () => {
    currentSearch = { highlight: TARGET_ID };
    mockUseApplications.mockReturnValue({ data: APPS, isLoading: false, isError: false });

    await act(async () => {
      render(<ApplicationsPage />);
    });

    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({ to: '/applications', search: {}, replace: true })
    );
  });

  it('un-collapses the stage section of the highlighted row', async () => {
    // Collapse the "applied" section so it would normally hide the target row.
    useSessionStore.setState((s) => ({
      applications: { ...s.applications, collapsedSections: ['applied'], filter: '' },
    }));
    currentSearch = { highlight: TARGET_ID };
    mockUseApplications.mockReturnValue({ data: APPS, isLoading: false, isError: false });

    await act(async () => {
      render(<ApplicationsPage />);
    });

    // After consuming highlight, the "applied" section must be un-collapsed in store...
    const { applications } = useSessionStore.getState();
    expect(applications.collapsedSections).not.toContain('applied');

    // ...AND the highlighted row must be present in the DOM (not hidden by collapse).
    // This confirms the section was actually un-collapsed in the rendered output,
    // not just in store state.
    const targetRow = screen
      .getAllByTestId('application-row')
      .find((el) => el.getAttribute('data-appid') === TARGET_ID);
    expect(targetRow).toBeDefined();
    expect(targetRow).toBeInTheDocument();
  });

  it('clears the local flash (highlighted=false) after 3.5s', async () => {
    currentSearch = { highlight: TARGET_ID };
    mockUseApplications.mockReturnValue({ data: APPS, isLoading: false, isError: false });

    await act(async () => {
      render(<ApplicationsPage />);
    });

    // Flash active right after mount.
    let targetRow = screen
      .getAllByTestId('application-row')
      .find((el) => el.getAttribute('data-appid') === TARGET_ID);
    expect(targetRow?.getAttribute('data-highlighted')).toBe('true');

    // Advance past the 3500ms timeout.
    await act(async () => {
      vi.advanceTimersByTime(3600);
    });

    targetRow = screen
      .getAllByTestId('application-row')
      .find((el) => el.getAttribute('data-appid') === TARGET_ID);
    expect(targetRow?.getAttribute('data-highlighted')).toBe('false');
  });

  it('does nothing when ?highlight is absent (inert path)', async () => {
    currentSearch = {};
    mockUseApplications.mockReturnValue({ data: APPS, isLoading: false, isError: false });

    await act(async () => {
      render(<ApplicationsPage />);
    });

    expect(mockNavigate).not.toHaveBeenCalled();
    const rows = screen.getAllByTestId('application-row');
    rows.forEach((row) => {
      expect(row.getAttribute('data-highlighted')).toBe('false');
    });
  });

  it('does not crash and clears the URL when ?highlight id does not match any application', async () => {
    // The highlight id is completely unknown — no application in the list has this id.
    currentSearch = { highlight: 'nonexistent-id' };
    mockUseApplications.mockReturnValue({ data: APPS, isLoading: false, isError: false });

    // Must not throw.
    await act(async () => {
      render(<ApplicationsPage />);
    });

    // URL param is cleared even when the id doesn't match (the useEffect consumes it unconditionally).
    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({ to: '/applications', search: {}, replace: true })
    );

    // No row is highlighted — the unknown id matches nothing.
    const rows = screen.getAllByTestId('application-row');
    rows.forEach((row) => {
      expect(row.getAttribute('data-highlighted')).toBe('false');
    });

    // No section was un-collapsed (nothing to find).
    const { applications } = useSessionStore.getState();
    expect(applications.collapsedSections).not.toContain('applied');
  });
});
