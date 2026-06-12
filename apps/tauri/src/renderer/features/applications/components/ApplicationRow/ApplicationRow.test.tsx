/**
 * ApplicationRow — status-change mutation + http(s) open-link gate (Gaps 5 & 6)
 *
 * Strategy:
 *  - Service hooks (`useSetApplicationStatus`, `useRemoveApplication`,
 *    `useOpenExternal`) are mocked at the module level — no AppClient /
 *    QueryClient provider tree needed.
 *  - `@ajh/ui` is imported real (Dropdown, ActionMenu, ConfirmModal are
 *    all exercised); only `useNotification` is stubbed if present.
 *  - `@ajh/translations` returns keys as-is.
 *  - The stale-detection functions (`isStale`, `staleDays`) depend on
 *    `Date.now()`. We fix `updatedAt` to a value in the very recent past so
 *    `isStale` always returns false and no stale badge appears — keeping
 *    assertions stable without fake timers.
 *
 * Gap 6 (security regression): the "open job link" action MUST be present for
 * an http(s) jobUrl and ABSENT for an empty / non-http(s) value. This locks in
 * the critical guard from commit 38290332.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';

import type { Application } from '@ajh/shared';

import { ApplicationRow } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── Service hooks ─────────────────────────────────────────────────────────────

const mockSetStatusMutateAsync = vi.fn().mockResolvedValue(undefined);
const mockRemoveMutateAsync = vi.fn().mockResolvedValue(undefined);
const mockOpenExternalMutate = vi.fn();

vi.mock('@/services', () => ({
  useSetApplicationStatus: () => ({
    mutateAsync: mockSetStatusMutateAsync,
    isPending: false,
  }),
  useRemoveApplication: () => ({
    mutateAsync: mockRemoveMutateAsync,
    isPending: false,
  }),
  useOpenExternal: () => ({
    mutate: mockOpenExternalMutate,
  }),
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

const RECENT_UPDATED_AT = Date.now() - 1000; // 1 second ago — never stale

function makeApp(overrides: Partial<Application>): Application {
  return {
    id: 'app-1',
    status: 'applied',
    createdAt: RECENT_UPDATED_AT,
    updatedAt: RECENT_UPDATED_AT,
    jobUrl: 'https://acme.com/job/1',
    board: 'linkedin',
    company: 'Acme',
    title: 'Engineer',
    candidate: 'Jane',
    answers: [],
    brief: '',
    notes: '',
    comp: '',
    contactName: '',
    contactEmail: '',
    ...overrides,
  };
}

// ── Reset mocks between tests ─────────────────────────────────────────────────

beforeEach(() => {
  mockSetStatusMutateAsync.mockClear();
  mockRemoveMutateAsync.mockClear();
  mockOpenExternalMutate.mockClear();
});

// ── Gap 5: status-change Dropdown calls setStatus mutation ──────────────

describe('ApplicationRow — status change', () => {
  it('changing the Dropdown calls setStatus.mutateAsync with the correct id and status', async () => {
    const app = makeApp({ id: 'app-42', status: 'applied' });
    render(<ApplicationRow application={app} />);

    // @ajh/ui Dropdown renders a <button aria-haspopup="listbox"> whose
    // accessible name is the currently selected option's label. Since t() returns
    // keys, the trigger is labelled "applications.status.applied".
    const trigger = screen.getByRole('button', {
      name: /applications\.status\.applied/i,
    });
    fireEvent.click(trigger);

    // After opening, the listbox options become visible.
    const listbox = await screen.findByRole('listbox');
    const interviewingOption = within(listbox).getByRole('option', {
      name: /applications\.status\.interviewing/i,
    });
    fireEvent.click(interviewingOption);

    expect(mockSetStatusMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockSetStatusMutateAsync).toHaveBeenCalledWith({
      id: 'app-42',
      status: 'interviewing',
    });
  });

  it('calls setStatus.mutateAsync with the correct status when selecting saved', async () => {
    const app = makeApp({ id: 'app-99', status: 'applied' });
    render(<ApplicationRow application={app} />);

    const trigger = screen.getByRole('button', {
      name: /applications\.status\.applied/i,
    });
    fireEvent.click(trigger);

    const listbox = await screen.findByRole('listbox');
    const savedOption = within(listbox).getByRole('option', {
      name: /applications\.status\.saved/i,
    });
    fireEvent.click(savedOption);

    expect(mockSetStatusMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockSetStatusMutateAsync).toHaveBeenCalledWith({
      id: 'app-99',
      status: 'saved',
    });
  });
});

// ── Gap 6: http(s) open-link gate (security regression — commit 38290332) ─────

describe('ApplicationRow — open-link gate (security regression)', () => {
  it('renders the open-job-link action menu item for an https jobUrl', () => {
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });
    render(<ApplicationRow application={app} />);

    // Open the ActionMenu.
    const actionsBtn = screen.getByRole('button', { name: 'applications.row.actions' });
    fireEvent.click(actionsBtn);

    // The open-link item must be present.
    expect(screen.getByRole('menuitem', { name: 'applications.row.openUrl' })).toBeInTheDocument();
  });

  it('renders the open-job-link action menu item for an http jobUrl', () => {
    const app = makeApp({ jobUrl: 'http://acme.com/job/1' });
    render(<ApplicationRow application={app} />);

    const actionsBtn = screen.getByRole('button', { name: 'applications.row.actions' });
    fireEvent.click(actionsBtn);

    expect(screen.getByRole('menuitem', { name: 'applications.row.openUrl' })).toBeInTheDocument();
  });

  it('does NOT render the open-job-link action for an empty jobUrl', () => {
    const app = makeApp({ jobUrl: '' });
    render(<ApplicationRow application={app} />);

    const actionsBtn = screen.getByRole('button', { name: 'applications.row.actions' });
    fireEvent.click(actionsBtn);

    expect(
      screen.queryByRole('menuitem', { name: 'applications.row.openUrl' })
    ).not.toBeInTheDocument();
  });

  it('does NOT render the open-job-link action for a javascript: jobUrl (dangerous scheme)', () => {
    // This is the critical regression: a javascript: url must never produce a
    // clickable "open" item — the guard is /^https?:\/\//i in ApplicationRow.
    const app = makeApp({ jobUrl: 'javascript:alert(1)' });
    render(<ApplicationRow application={app} />);

    const actionsBtn = screen.getByRole('button', { name: 'applications.row.actions' });
    fireEvent.click(actionsBtn);

    expect(
      screen.queryByRole('menuitem', { name: 'applications.row.openUrl' })
    ).not.toBeInTheDocument();
  });

  it('does NOT render the open-job-link action for a data: jobUrl (dangerous scheme)', () => {
    const app = makeApp({ jobUrl: 'data:text/html,<script>alert(1)</script>' });
    render(<ApplicationRow application={app} />);

    const actionsBtn = screen.getByRole('button', { name: 'applications.row.actions' });
    fireEvent.click(actionsBtn);

    expect(
      screen.queryByRole('menuitem', { name: 'applications.row.openUrl' })
    ).not.toBeInTheDocument();
  });

  it('does NOT render the open-job-link action for a file: jobUrl (dangerous scheme)', () => {
    const app = makeApp({ jobUrl: 'file:///etc/passwd' });
    render(<ApplicationRow application={app} />);

    const actionsBtn = screen.getByRole('button', { name: 'applications.row.actions' });
    fireEvent.click(actionsBtn);

    expect(
      screen.queryByRole('menuitem', { name: 'applications.row.openUrl' })
    ).not.toBeInTheDocument();
  });
});

// ── Gap 5 (MEDIUM): delete flow — keepDocuments=true and =false ───────────────
//
// The ActionMenu has two delete items:
//   "applications.row.deleteKeepDocs"  → handleDelete(true)  → keepDocs=true
//   "applications.row.deleteAll"       → handleDelete(false) → keepDocs=false
// Clicking either opens a ConfirmModal; confirming calls remove.mutateAsync with
// { id, keepDocuments: <bool> }.

describe('ApplicationRow — delete flow', () => {
  it('keepDocuments=true: clicking "deleteKeepDocs" then confirming calls remove with keepDocuments:true', async () => {
    const app = makeApp({ id: 'app-del-1' });
    render(<ApplicationRow application={app} />);

    // Open ActionMenu.
    fireEvent.click(screen.getByRole('button', { name: 'applications.row.actions' }));

    // Click the "keep docs" delete item.
    fireEvent.click(
      await screen.findByRole('menuitem', { name: 'applications.row.deleteKeepDocs' })
    );

    // ConfirmModal should now be open — confirm it.
    const confirmBtn = await screen.findByRole('button', { name: 'applications.delete.confirm' });
    fireEvent.click(confirmBtn);

    expect(mockRemoveMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockRemoveMutateAsync).toHaveBeenCalledWith({ id: 'app-del-1', keepDocuments: true });
  });

  it('keepDocuments=false: clicking "deleteAll" then confirming calls remove with keepDocuments:false', async () => {
    const app = makeApp({ id: 'app-del-2' });
    render(<ApplicationRow application={app} />);

    fireEvent.click(screen.getByRole('button', { name: 'applications.row.actions' }));
    fireEvent.click(await screen.findByRole('menuitem', { name: 'applications.row.deleteAll' }));

    const confirmBtn = await screen.findByRole('button', { name: 'applications.delete.confirm' });
    fireEvent.click(confirmBtn);

    expect(mockRemoveMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockRemoveMutateAsync).toHaveBeenCalledWith({ id: 'app-del-2', keepDocuments: false });
  });
});

// ── Gap 6 (MEDIUM): nextActionAt badge — deterministic with vi.setSystemTime ───
//
// `nextActionLabel` compares `nextActionAt` to `Date.now()`.
// We fix the clock so tests are stable regardless of machine speed.

describe('ApplicationRow — nextActionAt badge', () => {
  const FIXED_NOW = 1_700_000_000_000; // arbitrary fixed epoch ms

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(FIXED_NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders the "overdue" badge when nextActionAt is in the past', () => {
    const app = makeApp({ nextActionAt: FIXED_NOW - 1, updatedAt: FIXED_NOW });
    render(<ApplicationRow application={app} />);
    expect(screen.getByText('applications.row.overdue')).toBeInTheDocument();
    expect(screen.queryByText('applications.row.followUp')).not.toBeInTheDocument();
  });

  it('renders the "upcoming" (followUp) badge when nextActionAt is in the future', () => {
    const app = makeApp({ nextActionAt: FIXED_NOW + 86_400_000, updatedAt: FIXED_NOW });
    render(<ApplicationRow application={app} />);
    expect(screen.getByText('applications.row.followUp')).toBeInTheDocument();
    expect(screen.queryByText('applications.row.overdue')).not.toBeInTheDocument();
  });

  it('renders no nextAction badge when nextActionAt is unset', () => {
    const app = makeApp({ nextActionAt: undefined, updatedAt: FIXED_NOW });
    render(<ApplicationRow application={app} />);
    expect(screen.queryByText('applications.row.overdue')).not.toBeInTheDocument();
    expect(screen.queryByText('applications.row.followUp')).not.toBeInTheDocument();
  });
});
