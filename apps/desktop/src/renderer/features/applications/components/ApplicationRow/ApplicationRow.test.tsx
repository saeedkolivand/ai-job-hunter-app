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
const mockOpenExternalMutate = vi.fn();

vi.mock('@/services', () => ({
  useSetApplicationStatus: () => ({
    mutate: mockSetStatusMutate,
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
    jobDescription: '',
    jobSummary: '',
    contactName: '',
    contactEmail: '',
    ...overrides,
  };
}

// ── Reset mocks between tests ─────────────────────────────────────────────────

beforeEach(() => {
  mockSetStatusMutate.mockClear();
  mockSetStatusMutate.mockImplementation((_vars: unknown, options?: MutateOptions) => {
    options?.onSuccess?.();
  });
  mockRemoveMutateAsync.mockClear();
  mockOpenExternalMutate.mockClear();
  mockNavigate.mockClear();
});

/** Opens the stage Dropdown and picks `option` (the i18n key fragment). */
async function changeStage(currentStatus: string, option: string) {
  fireEvent.click(
    screen.getByRole('button', {
      name: new RegExp(`applications\\.status\\.${currentStatus}`, 'i'),
    })
  );
  const listbox = await screen.findByRole('listbox');
  fireEvent.click(
    within(listbox).getByRole('option', {
      name: new RegExp(`applications\\.status\\.${option}`, 'i'),
    })
  );
}

// ── Gap 5: status-change Dropdown calls setStatus mutation ──────────────

describe('ApplicationRow — status change', () => {
  it('changing the Dropdown calls setStatus.mutate with the correct id and status', async () => {
    const app = makeApp({ id: 'app-42', status: 'applied' });
    render(<ApplicationRow application={app} />);

    // @ajh/ui Dropdown renders a <button aria-haspopup="listbox"> whose
    // accessible name is the currently selected option's label. Since t() returns
    // keys, the trigger is labelled "applications.status.applied".
    await changeStage('applied', 'interviewing');

    expect(mockSetStatusMutate).toHaveBeenCalledTimes(1);
    expect(mockSetStatusMutate.mock.calls[0]?.[0]).toEqual({
      id: 'app-42',
      status: 'interviewing',
    });
  });

  it('calls setStatus.mutate with the correct status when selecting saved', async () => {
    const app = makeApp({ id: 'app-99', status: 'applied' });
    render(<ApplicationRow application={app} />);

    await changeStage('applied', 'saved');

    expect(mockSetStatusMutate).toHaveBeenCalledTimes(1);
    expect(mockSetStatusMutate.mock.calls[0]?.[0]).toEqual({
      id: 'app-99',
      status: 'saved',
    });
  });

  it('surfaces a localized inline error (and NO note callback) when the mutation fails', async () => {
    mockSetStatusMutate.mockImplementation((_vars: unknown, options?: MutateOptions) => {
      options?.onError?.();
    });
    const onStatusChanged = vi.fn();
    render(
      <ApplicationRow
        application={makeApp({ id: 'app-err', status: 'applied' })}
        onStatusChanged={onStatusChanged}
      />
    );

    await changeStage('applied', 'offer');

    expect(screen.getByRole('alert')).toHaveTextContent('applications.row.statusError');
    expect(onStatusChanged).not.toHaveBeenCalled();
  });

  // Dropdown.select fires onChange even when the CURRENT option is re-picked.
  it('re-picking the current stage writes nothing and raises no note prompt', async () => {
    const onStatusChanged = vi.fn();
    render(
      <ApplicationRow
        application={makeApp({ id: 'app-noop', status: 'applied' })}
        onStatusChanged={onStatusChanged}
      />
    );

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
    render(
      <ApplicationRow
        application={makeApp({ id: 'app-note', status: 'applied' })}
        onStatusChanged={onStatusChanged}
      />
    );

    await changeStage('applied', 'interviewing');

    expect(onStatusChanged).toHaveBeenCalledTimes(1);
    expect(onStatusChanged).toHaveBeenCalledWith('interviewing');
  });

  it('renders no dialog of its own (state here would not survive the refetch)', async () => {
    render(<ApplicationRow application={makeApp({ id: 'app-note-2', status: 'applied' })} />);
    await changeStage('applied', 'interviewing');

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});

// ── Richer row meta — board chip, salary, date stamp ──────────────────────────

describe('ApplicationRow — row meta', () => {
  it('renders the localized board chip for a known board id', () => {
    render(<ApplicationRow application={makeApp({ board: 'linkedin' })} />);
    expect(screen.getByText('jobs.boards.linkedin')).toBeInTheDocument();
  });

  it('renders no board chip when the board is blank', () => {
    const { container } = render(<ApplicationRow application={makeApp({ board: '   ' })} />);
    expect(container.textContent).not.toContain('jobs.boards.');
  });

  it('renders a currency-formatted salary range when the posting carried one', () => {
    render(
      <ApplicationRow
        application={makeApp({ salaryMin: 60000, salaryMax: 80000, salaryCurrency: 'EUR' })}
      />
    );
    // Locale-agnostic assertion: both bounds and the en-dash separator are present.
    const salary = screen.getByText(/60[,. ]?000.*–.*80[,. ]?000/);
    expect(salary).toBeInTheDocument();
  });

  it('renders no salary text when the posting carried none', () => {
    const { container } = render(<ApplicationRow application={makeApp({})} />);
    expect(container.textContent).not.toMatch(/\d{2}[,. ]?\d{3}/);
  });

  it('labels the stamp "applied" when appliedAt is set and "updated" otherwise', () => {
    const { unmount } = render(
      <ApplicationRow application={makeApp({ appliedAt: RECENT_UPDATED_AT })} />
    );
    expect(screen.getByText('applications.row.appliedAgo')).toBeInTheDocument();
    unmount();

    render(<ApplicationRow application={makeApp({ appliedAt: undefined })} />);
    expect(screen.getByText('applications.row.updatedAgo')).toBeInTheDocument();
  });

  it('shows the per-stage Tag only when showStageTag is set', () => {
    // The stage label always appears once as the Dropdown trigger's own label,
    // so the Tag is the SECOND occurrence — count rather than presence.
    const { unmount } = render(
      <ApplicationRow application={makeApp({ status: 'rejected' })} showStageTag />
    );
    expect(screen.getAllByText('applications.status.rejected')).toHaveLength(2);
    unmount();

    render(<ApplicationRow application={makeApp({ status: 'rejected' })} />);
    expect(screen.getAllByText('applications.status.rejected')).toHaveLength(1);
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

// ── Row navigation — clicking the row body navigates to the detail route ───────

describe('ApplicationRow — row navigation', () => {
  it('clicking the row body navigates to the detail route', () => {
    const app = makeApp({ id: 'app-nav-1' });
    render(<ApplicationRow application={app} />);

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
    const app = makeApp({ id: 'app-nav-2' });
    render(<ApplicationRow application={app} />);

    // The ActionMenu trigger is wrapped in a stopPropagation div — clicks on
    // it must not bubble to the row's openDetail handler.
    const actionsBtn = screen.getByRole('button', { name: 'applications.row.actions' });
    fireEvent.click(actionsBtn);

    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it('clicking the status Dropdown does NOT navigate', () => {
    const app = makeApp({ id: 'app-nav-5', status: 'applied' });
    render(<ApplicationRow application={app} />);

    // The status Dropdown trigger is wrapped in a stopPropagation div — clicks
    // on it must not bubble to the row's openDetail handler.
    const trigger = screen.getByRole('button', {
      name: /applications\.status\.applied/i,
    });
    fireEvent.click(trigger);

    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it('pressing Enter on the row navigates to the detail route', () => {
    const app = makeApp({ id: 'app-nav-3' });
    render(<ApplicationRow application={app} />);

    const rowButton = screen.getByRole('button', { name: 'applications.detail.openAria' });
    fireEvent.keyDown(rowButton, { key: 'Enter' });

    expect(mockNavigate).toHaveBeenCalledTimes(1);
    expect(mockNavigate).toHaveBeenCalledWith({
      to: '/applications/$id',
      params: { id: 'app-nav-3' },
      search: { from: 'applications' },
    });
  });

  it('pressing Space on the row navigates to the detail route', () => {
    const app = makeApp({ id: 'app-nav-4' });
    render(<ApplicationRow application={app} />);

    const rowButton = screen.getByRole('button', { name: 'applications.detail.openAria' });
    fireEvent.keyDown(rowButton, { key: ' ' });

    expect(mockNavigate).toHaveBeenCalledTimes(1);
    expect(mockNavigate).toHaveBeenCalledWith({
      to: '/applications/$id',
      params: { id: 'app-nav-4' },
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

// ── Post-change note affordance (the list's zero-keystroke alternative) ───────

describe('ApplicationRow — note chip', () => {
  it('renders the chip only when the page asks for it', () => {
    const { unmount } = render(<ApplicationRow application={makeApp({})} />);
    expect(
      screen.queryByRole('button', { name: 'applications.row.addNoteHint' })
    ).not.toBeInTheDocument();
    unmount();

    render(<ApplicationRow application={makeApp({})} showNoteHint />);
    expect(
      screen.getByRole('button', { name: 'applications.row.addNoteHint' })
    ).toBeInTheDocument();
  });

  it('clicking the chip asks for the note dialog WITHOUT navigating to the detail page', () => {
    const onAddNote = vi.fn();
    render(<ApplicationRow application={makeApp({})} showNoteHint onAddNote={onAddNote} />);

    fireEvent.click(screen.getByRole('button', { name: 'applications.row.addNoteHint' }));

    expect(onAddNote).toHaveBeenCalledTimes(1);
    // The chip sits inside the row's click target — it must stop propagation.
    expect(mockNavigate).not.toHaveBeenCalled();
  });
});
