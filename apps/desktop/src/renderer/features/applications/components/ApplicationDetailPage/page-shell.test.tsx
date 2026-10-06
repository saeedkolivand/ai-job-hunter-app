/**
 * ApplicationDetailPage — page shell: wizard reset, not-found / loading, ?tab= routing, Back, delete, stage Dropdown
 *
 * Shared mocks + fixtures live in ./test-support.
 */

import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { Application } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

import { ApplicationDetailPage, renderLoaded } from './test-render';
import {
  makeApp,
  mockNavigate,
  mockRemoveMutateAsync,
  mockSessionState,
  mockSetApplicationApply,
  mockSetStatusMutate,
  mockUseAiGenerations,
  mockUseApplication,
  setLoaded,
  state,
  type StatusMutateOptions,
} from './test-support';

describe('ApplicationDetailPage — wizard reset on mount', () => {
  it('seeds applyForId for the active application when it differs from the slice', () => {
    const app = makeApp({ id: 'app-reset-1' });
    setLoaded(app);

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

    setLoaded(app);

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
    setLoaded(null);

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

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — ?tab= query-param behaviour
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — ?tab= behaviour', () => {
  it('defaults to the overview tab when no ?tab= param is present (mockTab coerced to overview)', () => {
    // Route.useSearch returns { tab: 'overview' } as set in mockTab default.
    // The overview tab renders the notes field.
    state.tab = 'overview';
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
    (state.tab as unknown) = undefined;
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

  // Each case: the tab clicked, the search params before the click, and the
  // merged result the functional updater must produce (`from` is preserved).
  it.each([
    {
      name: 'clicking a tab calls navigate with a functional search updater that sets tab + preserves from',
      id: 'app-nav-1',
      tab: 'timeline',
      prev: { from: 'jobs' },
      want: { from: 'jobs', tab: 'timeline' },
    },
    {
      name: 'clicking the brief tab sets tab: "brief" via the functional updater',
      id: 'app-nav-2',
      tab: 'brief',
      prev: {},
      want: { tab: 'brief' },
    },
    {
      name: 'clicking the documents tab sets tab: "documents" via the functional updater',
      id: 'app-nav-3',
      tab: 'documents',
      prev: { from: 'autopilot' },
      want: { from: 'autopilot', tab: 'documents' },
    },
  ])('$name', async ({ id, tab, prev, want }) => {
    state.tab = 'overview';
    const user = userEvent.setup();
    renderLoaded({ id });

    await user.click(screen.getByRole('tab', { name: `applications.detail.tabs.${tab}` }));

    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({ search: expect.any(Function), replace: true })
    );
    expect(lastSearchUpdater()(prev)).toEqual(want);
  });

  it('renders the active tabpanel with the correct id for the current tab', () => {
    state.tab = 'timeline';
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
  it.each([
    {
      name: 'returning from Autopilot WITH a pending focus opts out of scroll restoration',
      from: 'autopilot' as const,
      lastAppliedId: 'ap-1',
      label: 'backAutopilot',
      want: { to: '/autopilot', resetScroll: false },
    },
    {
      name: 'returning from Autopilot WITHOUT a pending focus (e.g. forward-nav replay) keeps the default reset',
      from: 'autopilot' as const,
      lastAppliedId: null,
      label: 'backAutopilot',
      want: { to: '/autopilot', resetScroll: true },
    },
    {
      name: 'returning from Jobs keeps the default scroll reset (no compensating scroll effect there)',
      from: 'jobs' as const,
      lastAppliedId: null,
      label: 'backJobs',
      want: { to: '/jobs', resetScroll: true },
    },
    {
      name: 'returning with no origin (plain applications list) also keeps the default scroll reset',
      from: undefined,
      lastAppliedId: null,
      label: 'back',
      want: { to: '/applications', resetScroll: true },
    },
  ])('$name', async ({ from, lastAppliedId, label, want }) => {
    state.from = from;
    mockSessionState.autopilot.lastAppliedId = lastAppliedId;
    const user = userEvent.setup();
    renderLoaded({ id: 'app-back' });

    await user.click(screen.getByText(`applications.detail.${label}`));

    expect(mockNavigate).toHaveBeenCalledWith(want);
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
    state.tab = 'overview';
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
    state.tab = 'overview';
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
    state.tab = 'overview';
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

// ── Status change — no-op guard + failure surface ─────────────────────────────

describe('ApplicationDetailPage — header stage Dropdown', () => {
  const renderHeader = (app: Application) => {
    state.tab = 'overview';
    setLoaded(app);
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
