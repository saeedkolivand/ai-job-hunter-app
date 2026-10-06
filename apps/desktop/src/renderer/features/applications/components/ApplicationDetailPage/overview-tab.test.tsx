/**
 * ApplicationDetailPage — Overview tab: save-on-blur, follow-up, refetch, contact rejection
 *
 * Shared mocks + fixtures live in ./test-support.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { Application } from '@ajh/shared';

import { ApplicationDetailPage } from './test-render';
import {
  makeApp,
  mockApp,
  mockUpdateApplicationMutate,
  mockUseApplication,
  setLoaded,
  state,
} from './test-support';

/** Type `value` into the labelled field and blur it (the save-on-blur gesture). */
function editField(label: string, value: string) {
  const field = screen.getByLabelText(label);
  fireEvent.change(field, { target: { value } });
  fireEvent.blur(field);
  return field;
}

describe('ApplicationDetailPage — save-on-blur (Overview tab)', () => {
  function renderLoadedApp(app: Application) {
    state.tab = 'overview';
    setLoaded(app);
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

    editField('applications.detail.notesLabel', 'new note text');

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

    editField('applications.detail.contactNameLabel', 'Bob');

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
    state.tab = 'overview';
    setLoaded(makeApp({ id: 'app-wipe-1', contactName: '' }));
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

    editField('applications.detail.contactEmailLabel', 'b@b.com');

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

    editField('applications.detail.compLabel', '90k');

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

    editField('applications.detail.nextActionLabel', '');

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

// ── Follow-up promotion — visible from every tab, tinted when overdue ─────────

describe('ApplicationDetailPage — follow-up promotion', () => {
  const NOW = 1_700_000_000_000;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });
  afterEach(() => vi.useRealTimers());

  const renderWith = (app: Application) => {
    setLoaded(app);
    render(<ApplicationDetailPage />);
  };

  it('shows an overdue chip in the header (persists across tabs) when the date has passed', () => {
    state.tab = 'documents';
    renderWith(makeApp({ nextActionAt: NOW - 86_400_000 }));

    expect(screen.getByText('applications.detail.followUpOverdue')).toBeInTheDocument();
    expect(screen.queryByText('applications.detail.followUpDue')).not.toBeInTheDocument();
  });

  it('shows an upcoming chip when the date is still in the future', () => {
    state.tab = 'documents';
    renderWith(makeApp({ nextActionAt: NOW + 86_400_000 }));

    expect(screen.getByText('applications.detail.followUpDue')).toBeInTheDocument();
    expect(screen.queryByText('applications.detail.followUpOverdue')).not.toBeInTheDocument();
  });

  it('shows no chip at all when no reminder is set', () => {
    state.tab = 'documents';
    renderWith(makeApp({ nextActionAt: undefined }));

    expect(screen.queryByText('applications.detail.followUpDue')).not.toBeInTheDocument();
    expect(screen.queryByText('applications.detail.followUpOverdue')).not.toBeInTheDocument();
  });

  it('leads the Overview sheet with its own Follow-up section carrying the date field', () => {
    state.tab = 'overview';
    renderWith(makeApp({ nextActionAt: NOW - 86_400_000 }));

    expect(screen.getByText('applications.detail.followUpSection')).toBeInTheDocument();
    // The field itself still lives under its established label (deep links + the
    // save-on-blur tests above depend on it).
    expect(screen.getByLabelText('applications.detail.nextActionLabel')).toBeInTheDocument();
    expect(screen.getByText('applications.detail.followUpOverdueHint')).toBeInTheDocument();
  });

  it('uses the neutral hint when there is no reminder yet', () => {
    state.tab = 'overview';
    renderWith(makeApp({ nextActionAt: undefined }));

    expect(screen.getByText('applications.detail.followUpNoneHint')).toBeInTheDocument();
    expect(screen.queryByText('applications.detail.followUpOverdueHint')).not.toBeInTheDocument();
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
    state.tab = 'overview';
    setLoaded(app);
    return render(<ApplicationDetailPage />);
  };

  it('an uncommitted sibling edit and its caret survive another field write landing', () => {
    const { rerender } = renderOverview(makeApp({ id: 'app-live-1', notes: '', comp: '' }));

    // The user edits Notes and blurs it (write in flight), then types into Comp
    // WITHOUT blurring, leaving the caret there.
    editField('applications.detail.notesLabel', 'called the recruiter');

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
    mockApp(makeApp({ id: 'app-live-2', comp: '', updatedAt: 4242 }));
    rerender(<ApplicationDetailPage />);

    expect(screen.getByLabelText<HTMLInputElement>('applications.detail.compLabel').value).toBe(
      'draft only'
    );
  });
});

// ── Contact writes surface a rejected result (same field as ApplyByEmailTab) ───

describe('ApplicationDetailPage — contact write rejection', () => {
  const renderOverview = (app: Application) => {
    state.tab = 'overview';
    setLoaded(app);
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

    editField('applications.detail.contactEmailLabel', 'not-an-email');

    expect(screen.getByRole('alert')).toHaveTextContent('applications.detail.email.emailInvalid');
  });

  it('shows an alert when the backend rejects the contact name', () => {
    rejectWith('rejected');
    renderOverview(makeApp({ id: 'app-cn', contactName: '' }));

    editField('applications.detail.contactNameLabel', 'Dana');

    expect(screen.getByRole('alert')).toHaveTextContent('applications.detail.contactSaveError');
  });

  it('shows no alert when the write is accepted', () => {
    rejectWith(undefined);
    renderOverview(makeApp({ id: 'app-ok2', contactEmail: '' }));

    editField('applications.detail.contactEmailLabel', 'dana@acme.com');

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
