/**
 * ApplicationDetailPage — Timeline: notes, provisional email rows, Accept/Reject
 */

import { describe, expect, it } from 'vitest';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { StatusEvent } from '@ajh/shared';

import { renderTimeline } from './test-render';
import {
  makeEvent,
  mockAcceptStatusEventMutate,
  mockNotify,
  mockRejectStatusEventMutate,
  mockSetStatusMutate,
} from './test-support';

// ── Interaction log — the Timeline "Add note" entry point ────────────────────

/** Click the named button the way a user would. */
const press = (name: string) => userEvent.setup().click(screen.getByRole('button', { name }));

describe('ApplicationDetailPage — timeline notes', () => {
  const renderNotes = (events: StatusEvent[]) => renderTimeline(events, { status: 'interviewing' });

  it('"Add note" writes a SAME-status setStatus carrying the trimmed note', () => {
    renderNotes([]);

    fireEvent.click(screen.getByRole('button', { name: 'applications.note.add' }));
    fireEvent.change(screen.getByPlaceholderText('applications.note.placeholder'), {
      target: { value: '  Sent a thank-you email  ' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.note.save' }));

    expect(mockSetStatusMutate).toHaveBeenCalledTimes(1);
    expect(mockSetStatusMutate.mock.calls[0]?.[0]).toEqual({
      id: 'app-1',
      status: 'interviewing',
      note: 'Sent a thank-you email',
    });
  });

  it('the Add-note prompt uses the plain copy, not the post-transition copy', () => {
    renderNotes([]);

    fireEvent.click(screen.getByRole('button', { name: 'applications.note.add' }));

    expect(screen.getByText('applications.note.current')).toBeInTheDocument();
    expect(screen.queryByText('applications.note.afterChange')).not.toBeInTheDocument();
  });

  it('renders a same-status note event as ONE stage (never "X → X") with its note', () => {
    renderNotes([
      {
        eventId: 1,
        applicationId: 'app-1',
        fromStatus: 'interviewing',
        toStatus: 'interviewing',
        at: 1_700_000_000_000,
        note: 'Recruiter call booked',
        source: 'user',
        confirmed: true,
      },
    ]);

    // Scope to the tab panel: the header Dropdown also renders the stage label.
    const panel = within(screen.getByRole('tabpanel'));
    expect(panel.getByText('Recruiter call booked')).toBeInTheDocument();
    // A real transition renders the "from" label too; a note event must not.
    expect(panel.getAllByText('applications.status.interviewing')).toHaveLength(1);
  });

  it('still renders a real transition as from → to', () => {
    renderNotes([
      {
        eventId: 1,
        applicationId: 'app-1',
        fromStatus: 'applied',
        toStatus: 'interviewing',
        at: 1_700_000_000_000,
        note: '',
        source: 'user',
        confirmed: true,
      },
    ]);

    const panel = within(screen.getByRole('tabpanel'));
    expect(panel.getByText('applications.status.applied')).toBeInTheDocument();
    expect(panel.getByText('applications.status.interviewing')).toBeInTheDocument();
  });
});

describe('ApplicationDetailPage — Timeline: correction rows & ordering', () => {
  // `status_events.events()` orders by `at ASC, rowid ASC`; `eventId` IS the
  // rowid. `Array#sort` is STABLE, so a bare `b.at - a.at` keeps ascending
  // (input) order for any pair sharing one `at`, while every surrounding
  // pair sorts descending — reachable here because a reject appends its
  // reversal row immediately after its compare-and-set wins, so the
  // correction and the provisional row it resolves can share a millisecond.
  it('breaks a same-`at` tie using eventId, matching the backend’s at-ASC-rowid-ASC order', () => {
    renderTimeline([
      // Passed in backend order (oldest-inserted first), exactly like
      // `data.events` arrives from `applications_get`.
      makeEvent({
        eventId: 10,
        at: 1000,
        fromStatus: 'saved',
        toStatus: 'applied',
        source: 'user',
      }),
      makeEvent({
        eventId: 20,
        at: 2000,
        fromStatus: 'applied',
        toStatus: 'screening',
        source: 'email',
        confirmed: true,
      }),
      // The reversal row — same `at` as the row above, but a HIGHER
      // eventId/rowid, since it was inserted immediately after.
      makeEvent({
        eventId: 21,
        at: 2000,
        fromStatus: 'screening',
        toStatus: 'applied',
        source: 'email_reject',
        confirmed: true,
      }),
    ]);

    const list = screen.getByRole('list');
    const items = within(list).getAllByRole('listitem');
    expect(items).toHaveLength(3);

    // Newest-first display: the correction (eventId 21, the LATER of the two
    // same-`at` rows per the backend order) must render FIRST — ahead of the
    // row it resolves (eventId 20). A stable `b.at - a.at`-only sort keeps
    // the tie in ascending (input) order instead, rendering the correction
    // BELOW the very row it corrects.
    expect(items[0]?.textContent).toContain('applications.detail.timeline.correctionBadge');
    expect(items[1]?.textContent).not.toContain('applications.detail.timeline.correctionBadge');
    expect(items[2]?.textContent).toContain('applications.status.saved');
  });

  it('renders a provisional row without leaking the raw backend note, showing the localized hint instead', () => {
    renderTimeline([
      makeEvent({
        at: 1000,
        fromStatus: 'applied',
        toStatus: 'interviewing',
        source: 'email',
        confirmed: false,
        // The Rust auto-write's fixed, non-localized English literal.
        note: 'email-derived (unconfirmed)',
      }),
    ]);

    expect(screen.getByText('applications.detail.timeline.provisionalBadge')).toBeInTheDocument();
    expect(screen.getByText('applications.detail.timeline.provisionalHint')).toBeInTheDocument();
    // Never the raw backend literal, verbatim.
    expect(screen.queryByText('email-derived (unconfirmed)')).not.toBeInTheDocument();
  });
});

describe('ApplicationDetailPage — Timeline: provisional email rows', () => {
  it('renders Accept/Reject on the unconfirmed email row but not on a confirmed or user-sourced row', () => {
    renderTimeline([
      makeEvent({ at: 1000, fromStatus: 'saved', toStatus: 'applied', source: 'user' }),
      makeEvent({
        at: 1500,
        fromStatus: 'applied',
        toStatus: 'screening',
        source: 'email',
        confirmed: true, // an already-accepted email write — settled, no actions
      }),
      makeEvent({
        at: 2000,
        fromStatus: 'screening',
        toStatus: 'interviewing',
        source: 'email',
        confirmed: false, // the ONE provisional row
      }),
    ]);

    // Exactly one row is provisional → exactly one Accept/Reject pair, not one
    // per row (a bare "Accept"/"Reject" repeated down the list would be the
    // accessibility failure this guards against).
    expect(
      screen.getAllByRole('button', { name: 'applications.detail.timeline.acceptAria' })
    ).toHaveLength(1);
    expect(
      screen.getAllByRole('button', { name: 'applications.detail.timeline.rejectAria' })
    ).toHaveLength(1);
    expect(screen.getByText('applications.detail.timeline.provisionalBadge')).toBeInTheDocument();
  });

  it('does not render Accept/Reject when there is no unconfirmed email row at all', () => {
    renderTimeline([
      makeEvent({ at: 1000, source: 'user' }),
      makeEvent({ at: 2000, source: 'email', confirmed: true }),
    ]);

    expect(
      screen.queryByRole('button', { name: 'applications.detail.timeline.acceptAria' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'applications.detail.timeline.rejectAria' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText('applications.detail.timeline.provisionalBadge')
    ).not.toBeInTheDocument();
  });

  it('clicking Accept calls acceptStatusEvent.mutate with the application id and the row eventId', async () => {
    renderTimeline([makeEvent({ eventId: 42, source: 'email', confirmed: false })], {
      id: 'app-accept-1',
    });

    await press('applications.detail.timeline.acceptAria');

    expect(mockAcceptStatusEventMutate).toHaveBeenCalledWith(
      { id: 'app-accept-1', eventId: 42 },
      expect.any(Object)
    );
    expect(mockNotify.success).toHaveBeenCalledWith({
      message: 'applications.detail.timeline.acceptSuccess',
    });
  });

  it('clicking Reject calls rejectStatusEvent.mutate with the application id and the row eventId', async () => {
    renderTimeline([makeEvent({ eventId: 77, source: 'email', confirmed: false })], {
      id: 'app-reject-1',
    });

    await press('applications.detail.timeline.rejectAria');

    expect(mockRejectStatusEventMutate).toHaveBeenCalledWith(
      { id: 'app-reject-1', eventId: 77 },
      expect.any(Object)
    );
  });

  // ── Regression: two provisional rows can coexist on the ordinary happy path
  // (a confirmation email, then a later rejection email, both still
  // unreviewed) — the shipped bug resolved Accept/Reject to whichever row the
  // BACKEND considered "most recent", regardless of which row's button was
  // actually clicked, because every row shared ONE closure with no per-row
  // identity. A single-row fixture can never catch this class — these two
  // tests must stay permanently, with two DISTINCT eventIds asserted.
  it('clicking Accept on the OLDER of two provisional rows targets that eventId, not the newer eventId', async () => {
    const user = userEvent.setup();
    const older = makeEvent({
      eventId: 101,
      at: 1000,
      fromStatus: 'saved',
      toStatus: 'applied',
      source: 'email',
      confirmed: false,
    });
    const newer = makeEvent({
      eventId: 202,
      at: 2000,
      fromStatus: 'applied',
      toStatus: 'rejected',
      source: 'email',
      confirmed: false,
    });
    // orderedEvents sorts newest-first, so the DOM order is [newer, older].
    renderTimeline([older, newer], { id: 'app-two-provisional-accept' });

    const acceptButtons = screen.getAllByRole('button', {
      name: 'applications.detail.timeline.acceptAria',
    });
    expect(acceptButtons).toHaveLength(2);

    // Click the OLDER row's button — the second one in DOM order.
    const olderAcceptButton = acceptButtons[1];
    if (!olderAcceptButton) throw new Error('expected two Accept buttons');
    await user.click(olderAcceptButton);

    expect(mockAcceptStatusEventMutate).toHaveBeenCalledWith(
      { id: 'app-two-provisional-accept', eventId: 101 },
      expect.any(Object)
    );
    expect(mockAcceptStatusEventMutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ eventId: 202 }),
      expect.any(Object)
    );
  });

  it('clicking Reject on the NEWER of two provisional rows targets that eventId, not the older eventId', async () => {
    const user = userEvent.setup();
    const older = makeEvent({
      eventId: 303,
      at: 1000,
      fromStatus: 'saved',
      toStatus: 'applied',
      source: 'email',
      confirmed: false,
    });
    const newer = makeEvent({
      eventId: 404,
      at: 2000,
      fromStatus: 'applied',
      toStatus: 'rejected',
      source: 'email',
      confirmed: false,
    });
    // orderedEvents sorts newest-first, so the DOM order is [newer, older].
    renderTimeline([older, newer], { id: 'app-two-provisional-reject' });

    const rejectButtons = screen.getAllByRole('button', {
      name: 'applications.detail.timeline.rejectAria',
    });
    expect(rejectButtons).toHaveLength(2);

    // Click the NEWER row's button — the first one in DOM order.
    const newerRejectButton = rejectButtons[0];
    if (!newerRejectButton) throw new Error('expected two Reject buttons');
    await user.click(newerRejectButton);

    expect(mockRejectStatusEventMutate).toHaveBeenCalledWith(
      { id: 'app-two-provisional-reject', eventId: 404 },
      expect.any(Object)
    );
    expect(mockRejectStatusEventMutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ eventId: 303 }),
      expect.any(Object)
    );
  });
});
