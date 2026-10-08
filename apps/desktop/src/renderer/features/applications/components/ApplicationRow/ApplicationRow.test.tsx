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

import { makeApplication } from '@/features/applications/lib/test-fixtures';

import { ApplicationRow } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── Router — render standalone (no RouterProvider) ────────────────────────────

const mockNavigate = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

// ── Service hooks ─────────────────────────────────────────────────────────────

/**
 * `setStatus.mutate(vars, options)` — the fake resolves SUCCESSFULLY by invoking
 * `options.onSuccess`, which is what opens the optional-note prompt. Individual
 * tests can re-implement it to exercise the error branch.
 */
type MutateOptions = { onSuccess?: () => void; onError?: () => void };
const mockSetStatusMutate = vi.fn((_vars: unknown, options?: MutateOptions) => {
  options?.onSuccess?.();
});
const mockRemoveMutateAsync = vi.fn().mockResolvedValue(undefined);

vi.mock('@/services', () => ({
  useSetApplicationStatus: () => ({
    mutate: mockSetStatusMutate,
    isPending: false,
  }),
  useRemoveApplication: () => ({
    mutateAsync: mockRemoveMutateAsync,
    isPending: false,
  }),
  useOpenExternal: () => ({ mutate: vi.fn() }),
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

const RECENT_UPDATED_AT = Date.now() - 1000; // 1 second ago — never stale

const makeApp = (overrides: Partial<Application>) =>
  makeApplication({ createdAt: RECENT_UPDATED_AT, updatedAt: RECENT_UPDATED_AT, ...overrides });

// ── Reset mocks between tests ─────────────────────────────────────────────────

beforeEach(() => {
  mockSetStatusMutate.mockClear();
  mockSetStatusMutate.mockImplementation((_vars: unknown, options?: MutateOptions) => {
    options?.onSuccess?.();
  });
  mockRemoveMutateAsync.mockClear();
  mockNavigate.mockClear();
});

const renderRow = (
  overrides: Partial<Application> = {},
  props: Partial<React.ComponentProps<typeof ApplicationRow>> = {}
) => render(<ApplicationRow application={makeApp(overrides)} {...props} />);

/** Opens the stage Dropdown and picks `option` (the i18n key fragment). */
async function changeStage(currentStatus: string, option: string) {
  const stage = (status: string) => ({
    name: new RegExp(`applications\\.status\\.${status}`, 'i'),
  });
  fireEvent.click(screen.getByRole('button', stage(currentStatus)));
  const listbox = await screen.findByRole('listbox');
  fireEvent.click(within(listbox).getByRole('option', stage(option)));
}

// ── Gap 5: status-change Dropdown calls setStatus mutation ──────────────

describe('ApplicationRow — status change', () => {
  // @ajh/ui Dropdown renders a <button aria-haspopup="listbox"> whose accessible
  // name is the currently selected option's label. Since t() returns keys, the
  // trigger is labelled "applications.status.applied".
  it.each([
    {
      name: 'changing the Dropdown calls setStatus.mutate with the correct id and status',
      id: 'app-42',
      to: 'interviewing',
    },
    {
      name: 'calls setStatus.mutate with the correct status when selecting saved',
      id: 'app-99',
      to: 'saved',
    },
  ])('$name', async ({ id, to }) => {
    renderRow({ id, status: 'applied' });

    await changeStage('applied', to);

    expect(mockSetStatusMutate).toHaveBeenCalledTimes(1);
    expect(mockSetStatusMutate.mock.calls[0]?.[0]).toEqual({ id, status: to });
  });

  it('surfaces a localized inline error (and NO note callback) when the mutation fails', async () => {
    mockSetStatusMutate.mockImplementation((_vars: unknown, options?: MutateOptions) => {
      options?.onError?.();
    });
    const onStatusChanged = vi.fn();
    renderRow({ id: 'app-err', status: 'applied' }, { onStatusChanged });

    await changeStage('applied', 'offer');

    expect(screen.getByRole('alert')).toHaveTextContent('applications.row.statusError');
    expect(onStatusChanged).not.toHaveBeenCalled();
  });

  // Dropdown.select fires onChange even when the CURRENT option is re-picked.
  it('re-picking the current stage writes nothing and raises no note prompt', async () => {
    const onStatusChanged = vi.fn();
    renderRow({ id: 'app-noop', status: 'applied' }, { onStatusChanged });

    await changeStage('applied', 'applied');

    expect(mockSetStatusMutate).not.toHaveBeenCalled();
    expect(onStatusChanged).not.toHaveBeenCalled();
  });
});

// ── Optional status note — the row only REPORTS a persisted change ───────────
//
// The prompt deliberately does NOT live here: the invalidation refetch that
// follows the write re-sections the list and unmounts this row, taking any local
// prompt state with it. The row raises `onStatusChanged`; the page owns the
// dialog (see ApplicationsPage.notes.test.tsx, which drives the refetch).

describe('ApplicationRow — status note handoff', () => {
  it('reports the new stage to the page after a successful change', async () => {
    const onStatusChanged = vi.fn();
    renderRow({ id: 'app-note', status: 'applied' }, { onStatusChanged });

    await changeStage('applied', 'interviewing');

    expect(onStatusChanged).toHaveBeenCalledTimes(1);
    expect(onStatusChanged).toHaveBeenCalledWith('interviewing');
  });

  it('renders no dialog of its own (state here would not survive the refetch)', async () => {
    renderRow({ id: 'app-note-2', status: 'applied' });
    await changeStage('applied', 'interviewing');

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});

// ── Richer row meta — board chip, salary, date stamp ──────────────────────────

describe('ApplicationRow — row meta', () => {
  it('renders the localized board chip for a known board id', () => {
    renderRow({ board: 'linkedin' });
    expect(screen.getByText('jobs.boards.linkedin')).toBeInTheDocument();
  });

  it('renders no board chip when the board is blank', () => {
    const { container } = renderRow({ board: '   ' });
    expect(container.textContent).not.toContain('jobs.boards.');
  });

  it('renders a currency-formatted salary range when the posting carried one', () => {
    renderRow({ salaryMin: 60000, salaryMax: 80000, salaryCurrency: 'EUR' });
    // Locale-agnostic assertion: both bounds and the en-dash separator are present.
    const salary = screen.getByText(/60[,. ]?000.*–.*80[,. ]?000/);
    expect(salary).toBeInTheDocument();
  });

  it('renders no salary text when the posting carried none', () => {
    const { container } = renderRow({});
    expect(container.textContent).not.toMatch(/\d{2}[,. ]?\d{3}/);
  });

  it('labels the stamp "applied" when appliedAt is set and "updated" otherwise', () => {
    const { unmount } = renderRow({ appliedAt: RECENT_UPDATED_AT });
    expect(screen.getByText('applications.row.appliedAgo')).toBeInTheDocument();
    unmount();

    renderRow({ appliedAt: undefined });
    expect(screen.getByText('applications.row.updatedAgo')).toBeInTheDocument();
  });

  it('does not keep the "applied" stamp after a demotion back to saved', () => {
    renderRow({ status: 'saved', appliedAt: RECENT_UPDATED_AT });
    expect(screen.queryByText('applications.row.appliedAgo')).not.toBeInTheDocument();
    expect(screen.getByText('applications.row.updatedAgo')).toBeInTheDocument();
  });

  it('shows the per-stage Tag only when showStageTag is set', () => {
    // The stage label always appears once as the Dropdown trigger's own label,
    // so the Tag is the SECOND occurrence — count rather than presence.
    const { unmount } = renderRow({ status: 'rejected' }, { showStageTag: true });
    expect(screen.getAllByText('applications.status.rejected')).toHaveLength(2);
    unmount();

    renderRow({ status: 'rejected' });
    expect(screen.getAllByText('applications.status.rejected')).toHaveLength(1);
  });
});

// ── Gap 6: http(s) open-link gate (security regression — commit 38290332) ─────

describe('ApplicationRow — open-link gate (security regression)', () => {
  const openActions = (jobUrl: string) => {
    renderRow({ jobUrl });
    fireEvent.click(screen.getByRole('button', { name: 'applications.row.actions' }));
  };
  const openItem = { name: 'applications.row.openUrl' };

  it.each([
    ['an https jobUrl', 'https://acme.com/job/1'],
    ['an http jobUrl', 'http://acme.com/job/1'],
  ])('renders the open-job-link action menu item for %s', (_label, jobUrl) => {
    openActions(jobUrl);
    expect(screen.getByRole('menuitem', openItem)).toBeInTheDocument();
  });

  // The critical regression: a javascript: url must never produce a clickable
  // "open" item — the guard is /^https?:///i in ApplicationRow.
  it.each([
    ['an empty jobUrl', ''],
    ['a javascript: jobUrl (dangerous scheme)', 'javascript:alert(1)'],
    ['a data: jobUrl (dangerous scheme)', 'data:text/html,<script>alert(1)</script>'],
    ['a file: jobUrl (dangerous scheme)', 'file:///etc/passwd'],
  ])('does NOT render the open-job-link action for %s', (_label, jobUrl) => {
    openActions(jobUrl);
    expect(screen.queryByRole('menuitem', openItem)).not.toBeInTheDocument();
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
  it.each([
    {
      name: 'keepDocuments=true: clicking "deleteKeepDocs" then confirming calls remove with keepDocuments:true',
      id: 'app-del-1',
      item: 'applications.row.deleteKeepDocs',
      keepDocuments: true,
    },
    {
      name: 'keepDocuments=false: clicking "deleteAll" then confirming calls remove with keepDocuments:false',
      id: 'app-del-2',
      item: 'applications.row.deleteAll',
      keepDocuments: false,
    },
  ])('$name', async ({ id, item, keepDocuments }) => {
    renderRow({ id });

    fireEvent.click(screen.getByRole('button', { name: 'applications.row.actions' }));
    fireEvent.click(await screen.findByRole('menuitem', { name: item }));

    // ConfirmModal should now be open — confirm it.
    fireEvent.click(await screen.findByRole('button', { name: 'applications.delete.confirm' }));

    expect(mockRemoveMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockRemoveMutateAsync).toHaveBeenCalledWith({ id, keepDocuments });
  });
});

// ── Row navigation — clicking the row body navigates to the detail route ───────

describe('ApplicationRow — row navigation', () => {
  it('clicking the row body navigates to the detail route', () => {
    renderRow({ id: 'app-nav-1' });

    // The row itself has role="button" and an aria-label set via t(), which
    // returns the key string (t returns (key) => key). The label is
    // 'applications.detail.openAria' because the mock ignores interpolation params.
    const rowButton = screen.getByRole('button', { name: 'applications.detail.openAria' });
    fireEvent.click(rowButton);

    expect(mockNavigate).toHaveBeenCalledTimes(1);
    expect(mockNavigate).toHaveBeenCalledWith({
      to: '/applications/$id',
      params: { id: 'app-nav-1' },
      search: { from: 'applications' },
    });
  });

  it('clicking the actions (3-dots) menu does NOT navigate', () => {
    renderRow({ id: 'app-nav-2' });

    // The ActionMenu trigger is wrapped in a stopPropagation div — clicks on
    // it must not bubble to the row's openDetail handler.
    const actionsBtn = screen.getByRole('button', { name: 'applications.row.actions' });
    fireEvent.click(actionsBtn);

    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it('clicking the status Dropdown does NOT navigate', () => {
    renderRow({ id: 'app-nav-5', status: 'applied' });

    // The status Dropdown trigger is wrapped in a stopPropagation div — clicks
    // on it must not bubble to the row's openDetail handler.
    const trigger = screen.getByRole('button', {
      name: /applications\.status\.applied/i,
    });
    fireEvent.click(trigger);

    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it.each([
    ['Enter', 'Enter', 'app-nav-3'],
    ['Space', ' ', 'app-nav-4'],
  ])('pressing %s on the row navigates to the detail route', (_label, key, id) => {
    renderRow({ id });

    fireEvent.keyDown(screen.getByRole('button', { name: 'applications.detail.openAria' }), {
      key,
    });

    expect(mockNavigate).toHaveBeenCalledTimes(1);
    expect(mockNavigate).toHaveBeenCalledWith({
      to: '/applications/$id',
      params: { id },
      search: { from: 'applications' },
    });
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
    renderRow({ nextActionAt: FIXED_NOW - 86_400_000, updatedAt: FIXED_NOW });
    expect(screen.getByText('applications.row.overdue')).toBeInTheDocument();
    expect(screen.queryByText('applications.row.followUp')).not.toBeInTheDocument();
  });

  it('renders the "upcoming" (followUp) badge when nextActionAt is in the future', () => {
    renderRow({ nextActionAt: FIXED_NOW + 86_400_000, updatedAt: FIXED_NOW });
    expect(screen.getByText('applications.row.followUp')).toBeInTheDocument();
    expect(screen.queryByText('applications.row.overdue')).not.toBeInTheDocument();
  });

  it('renders no nextAction badge when nextActionAt is unset', () => {
    renderRow({ nextActionAt: undefined, updatedAt: FIXED_NOW });
    expect(screen.queryByText('applications.row.overdue')).not.toBeInTheDocument();
    expect(screen.queryByText('applications.row.followUp')).not.toBeInTheDocument();
  });
});

// ── Post-change note affordance (the list's zero-keystroke alternative) ───────

describe('ApplicationRow — note chip', () => {
  it('renders the chip only when the page asks for it', () => {
    const { unmount } = renderRow({});
    expect(
      screen.queryByRole('button', { name: 'applications.row.addNoteHint' })
    ).not.toBeInTheDocument();
    unmount();

    renderRow({}, { showNoteHint: true });
    expect(
      screen.getByRole('button', { name: 'applications.row.addNoteHint' })
    ).toBeInTheDocument();
  });

  // Enter/Space on a focused <button> fires a native click AND bubbles the
  // keydown — without stopPropagation the row's own handler opens the detail
  // page underneath the note dialog.
  it.each([
    ['Enter', 'Enter'],
    ['Space', ' '],
  ])('%s on the chip does NOT navigate to the detail page', (_label, key) => {
    renderRow({}, { showNoteHint: true, onAddNote: vi.fn() });

    fireEvent.keyDown(screen.getByRole('button', { name: 'applications.row.addNoteHint' }), {
      key,
    });

    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it('clicking the chip asks for the note dialog WITHOUT navigating to the detail page', () => {
    const onAddNote = vi.fn();
    renderRow({}, { showNoteHint: true, onAddNote });

    fireEvent.click(screen.getByRole('button', { name: 'applications.row.addNoteHint' }));

    expect(onAddNote).toHaveBeenCalledTimes(1);
    // The chip sits inside the row's click target — it must stop propagation.
    expect(mockNavigate).not.toHaveBeenCalled();
  });
});
