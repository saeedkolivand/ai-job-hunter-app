/**
 * ApplicationsPage — the optional status-note prompt ACROSS the invalidation
 * refetch.
 *
 * Why this file exists: a status write invalidates the applications list, and the
 * refetch moves the changed row into a different (keyed) stage section — React
 * unmounts the old row. A prompt owned by the row therefore vanished before the
 * user could type. These tests DELIVER that refetch (the mocked query data really
 * changes, then the tree re-renders) so the regression cannot come back silently.
 *
 * The real `ApplicationRow` is used — the handoff runs through the actual stage
 * Dropdown, not a stub.
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

// ── Router ────────────────────────────────────────────────────────────────────

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
}));

vi.mock('@/routes/applications.index', () => ({
  Route: { useSearch: () => ({}) },
}));

// ── Mutable query data — the refetch is simulated by mutating this ───────────

function makeApp(overrides: Partial<Application>): Application {
  return {
    id: 'app-1',
    status: 'applied',
    createdAt: 1000,
    updatedAt: 1000,
    jobUrl: '',
    board: '',
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

let apps: Application[] = [];
/** When true the NOTE write (the second call) fails; the transition still succeeds. */
let noteWriteFails = false;

type MutateOptions = { onSuccess?: () => void; onError?: () => void };
type StatusVars = { id: string; status: string; note?: string };

/**
 * Stands in for the real mutation + its invalidation: a transition write updates
 * the backing list (so the next render re-sections the row) before reporting
 * success, exactly as the server + refetch do.
 */
const mockSetStatusMutate = vi.fn((vars: StatusVars, options?: MutateOptions) => {
  const isNote = vars.note !== undefined;
  if (!isNote) {
    apps = apps.map((a) =>
      a.id === vars.id
        ? { ...a, status: vars.status as Application['status'], updatedAt: a.updatedAt + 1 }
        : a
    );
  }
  if (isNote && noteWriteFails) options?.onError?.();
  else options?.onSuccess?.();
});

const useApplicationsMock = () => ({ data: apps, isLoading: false, isError: false });
const useSetApplicationStatusMock = () => ({ mutate: mockSetStatusMutate, isPending: false });

// The page imports from `@/services/use-applications`; ApplicationRow imports the
// same hooks through the `@/services` barrel. Both must resolve to ONE spy.
vi.mock('@/services/use-applications', () => ({
  useApplications: () => useApplicationsMock(),
  useSetApplicationStatus: () => useSetApplicationStatusMock(),
}));

vi.mock('@/services', () => ({
  useSetApplicationStatus: () => useSetApplicationStatusMock(),
  useRemoveApplication: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  useOpenExternal: () => ({ mutate: vi.fn() }),
}));

// ── Page chrome stubs ─────────────────────────────────────────────────────────

vi.mock('@/features/applications/components/TrackJobModal', () => ({
  TrackJobModal: () => null,
}));

vi.mock('@/components/layout/PageShell', () => ({
  PageShell: ({ children, actions }: { children: React.ReactNode; actions?: React.ReactNode }) => (
    <div>
      {actions}
      {children}
    </div>
  ),
}));

// ── Setup ─────────────────────────────────────────────────────────────────────

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
  mockSetStatusMutate.mockClear();
  noteWriteFails = false;
  apps = [
    makeApp({ id: 'a1', status: 'applied', title: 'Alpha Role', company: 'Alpha' }),
    makeApp({ id: 'a2', status: 'applied', title: 'Beta Role', company: 'Beta' }),
  ];
});

/** Moves `a1` from `applied` to `interviewing` through the real row Dropdown. */
async function moveA1To(stage: string) {
  const trigger = screen.getAllByRole('button', { name: /applications\.status\.applied/i })[0];
  expect(trigger).toBeDefined();
  fireEvent.click(trigger as HTMLElement);
  const listbox = await screen.findByRole('listbox');
  fireEvent.click(
    within(listbox).getByRole('option', {
      name: new RegExp(`applications\\.status\\.${stage}`, 'i'),
    })
  );
}

/** Clicks the transient "Add a note?" chip the changed row offers. */
function openNotePrompt() {
  fireEvent.click(screen.getByRole('button', { name: 'applications.row.addNoteHint' }));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ApplicationsPage — status note across the refetch', () => {
  // A stage change on the LIST must not seize the screen: moving several rows is
  // a batch action, so the row offers a chip instead of a modal.
  it('offers a chip (no dialog) after a stage change, and the chip survives the refetch', async () => {
    const { rerender } = render(<ApplicationsPage />);

    // Capture a1's row node (both rows share an aria-label because `t` echoes
    // keys, so take the first — a1 leads the `applied` section) to prove it is
    // genuinely destroyed by the refetch.
    const rowBefore = screen.getAllByRole('button', {
      name: /applications\.detail\.openAria/i,
    })[0];
    expect(rowBefore).toBeDefined();

    await moveA1To('interviewing');
    // Deliver the refetch: the list now really reports a1 as `interviewing`, so
    // the row unmounts from the `applied` section and remounts under a new one.
    rerender(<ApplicationsPage />);

    // Proof the churn actually happened: a new stage section exists AND the old
    // row node is gone from the document — any state it had owned died with it.
    expect(
      screen
        .getAllByRole('button', { name: (n) => n.includes('applications.stages.interviewing') })
        .filter((el) => el.getAttribute('data-testid') !== TEST_IDS.applications.pipelineCard)
    ).toHaveLength(1);
    expect(rowBefore?.isConnected).toBe(false);

    // No modal was forced…
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    // …and the offer survived the churn because the PAGE holds it, keyed by id.
    expect(
      screen.getByRole('button', { name: 'applications.row.addNoteHint' })
    ).toBeInTheDocument();
  });

  it('the chip opens the note dialog, which also survives the refetch', async () => {
    const { rerender } = render(<ApplicationsPage />);
    await moveA1To('interviewing');
    rerender(<ApplicationsPage />);

    openNotePrompt();
    rerender(<ApplicationsPage />);

    expect(screen.getByRole('dialog')).toHaveAccessibleName('applications.note.title');
    expect(screen.getByPlaceholderText('applications.note.placeholder')).toBeInTheDocument();
    // The dialog names WHICH application the note lands on.
    expect(screen.getByText('Alpha · Alpha Role')).toBeInTheDocument();
  });

  it('only the changed row offers the chip', async () => {
    const { rerender } = render(<ApplicationsPage />);
    await moveA1To('interviewing');
    rerender(<ApplicationsPage />);

    expect(screen.getAllByRole('button', { name: 'applications.row.addNoteHint' })).toHaveLength(1);
  });

  it('saves a same-status note carrying the trimmed text, then closes', async () => {
    const { rerender } = render(<ApplicationsPage />);
    await moveA1To('interviewing');
    rerender(<ApplicationsPage />);
    openNotePrompt();

    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: '  Recruiter call booked  ' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.note.save' }));

    expect(mockSetStatusMutate).toHaveBeenCalledTimes(2);
    expect(mockSetStatusMutate.mock.calls[1]?.[0]).toEqual({
      id: 'a1',
      status: 'interviewing',
      note: 'Recruiter call booked',
    });
    rerender(<ApplicationsPage />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('writes the CURRENT stage, not the one captured when the prompt opened', async () => {
    const { rerender } = render(<ApplicationsPage />);
    await moveA1To('interviewing');
    rerender(<ApplicationsPage />);

    // Another surface (extension bridge / second window) moves it on again while
    // the prompt is open. The note must not revert that.
    apps = apps.map((a) => (a.id === 'a1' ? { ...a, status: 'offer', updatedAt: 5000 } : a));
    rerender(<ApplicationsPage />);
    openNotePrompt();

    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: 'Follow-up sent' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.note.save' }));

    expect(mockSetStatusMutate.mock.calls[1]?.[0]).toEqual({
      id: 'a1',
      status: 'offer',
      note: 'Follow-up sent',
    });
  });

  it('keeps the dialog open with the typed note when the note write FAILS', async () => {
    noteWriteFails = true;
    const { rerender } = render(<ApplicationsPage />);
    await moveA1To('interviewing');
    rerender(<ApplicationsPage />);
    openNotePrompt();

    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: 'Do not lose me' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.note.save' }));
    rerender(<ApplicationsPage />);

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('applications.note.saveError');
    expect(
      screen.getByPlaceholderText<HTMLTextAreaElement>('applications.note.placeholder').value
    ).toBe('Do not lose me');
  });

  it('Skip closes the prompt and writes nothing extra', async () => {
    const { rerender } = render(<ApplicationsPage />);
    await moveA1To('interviewing');
    rerender(<ApplicationsPage />);
    openNotePrompt();

    fireEvent.click(screen.getByRole('button', { name: 'applications.note.skip' }));
    rerender(<ApplicationsPage />);

    expect(mockSetStatusMutate).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('a fresh prompt starts empty (a skipped draft is not resurrected)', async () => {
    const { rerender } = render(<ApplicationsPage />);
    await moveA1To('interviewing');
    rerender(<ApplicationsPage />);
    openNotePrompt();

    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: 'abandoned draft' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.note.skip' }));
    rerender(<ApplicationsPage />);

    // Move the OTHER row so a second prompt opens.
    const trigger = screen.getAllByRole('button', { name: /applications\.status\.applied/i })[0];
    fireEvent.click(trigger as HTMLElement);
    const listbox = await screen.findByRole('listbox');
    fireEvent.click(within(listbox).getByRole('option', { name: /applications\.status\.offer/i }));
    rerender(<ApplicationsPage />);
    openNotePrompt();

    expect(
      screen.getByPlaceholderText<HTMLTextAreaElement>('applications.note.placeholder').value
    ).toBe('');
  });

  // Unsaved work: a stray click outside must not destroy the typed note.
  it('a backdrop click does NOT close the dialog', async () => {
    const { rerender } = render(<ApplicationsPage />);
    await moveA1To('interviewing');
    rerender(<ApplicationsPage />);
    openNotePrompt();

    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: 'still here' },
    });

    const dialog = screen.getByRole('dialog');
    const backdrop = dialog.parentElement as HTMLElement;
    fireEvent.mouseDown(backdrop);
    fireEvent.click(backdrop);

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText<HTMLTextAreaElement>('applications.note.placeholder').value
    ).toBe('still here');
  });
});
