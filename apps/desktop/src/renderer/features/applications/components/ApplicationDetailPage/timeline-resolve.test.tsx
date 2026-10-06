/**
 * ApplicationDetailPage — Timeline: reject outcomes, correction rows, in-flight rows
 *
 * Shared mocks + fixtures live in ./test-support.
 */

import { act } from 'react';
import { describe, expect, it } from 'vitest';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ApplicationDetailPage, renderTimeline } from './test-render';
import {
  makeApp,
  makeEvent,
  mockAcceptStatusEventMutate,
  mockNotify,
  mockRejectStatusEventMutate,
  mockUseApplication,
  type StatusEventMutateOptions,
} from './test-support';

/** Click the named button the way a user would. */
const press = (name: string) => userEvent.setup().click(screen.getByRole('button', { name }));

describe('ApplicationDetailPage — Timeline: reject outcomes & in-flight rows', () => {
  // The mutation result carries no `reverted` flag (`ApplicationMutationResult`
  // is just `{ error?: string }`), so the UI cannot know from the response
  // alone whether the compare-and-set actually reverted the status — only the
  // refetched events can say that. This pins BOTH halves: the toast copy used
  // is the honest one (never a "reverted" claim), and when the CAS lost (the
  // user changed the status by hand meanwhile) nothing rendered claims a
  // revert happened either.
  it('reject when the CAS loses does not claim the status was reverted — asserts only what renders', async () => {
    const provisional = makeEvent({
      at: 2000,
      fromStatus: 'applied',
      toStatus: 'interviewing',
      source: 'email',
      confirmed: false,
    });
    const { rerender } = renderTimeline([provisional], { id: 'app-reject-cas-lost' });

    await press('applications.detail.timeline.rejectAria');

    // The success copy never asserts a revert — it's deliberately neutral
    // ("reviewed"), because the mutation response can't say what happened.
    expect(mockNotify.success).toHaveBeenCalledWith({
      message: 'applications.detail.timeline.rejectSuccess',
    });
    expect(mockNotify.success).not.toHaveBeenCalledWith(
      expect.objectContaining({ message: expect.stringMatching(/revert/i) })
    );

    // Simulate the CAS-lost outcome: the backend marks the original row
    // reviewed (`confirmed: true`) but appends NO reversal row — the status
    // the user set by hand in the meantime is left untouched.
    mockUseApplication.mockReturnValue({
      data: {
        application: makeApp({ id: 'app-reject-cas-lost' }),
        events: [{ ...provisional, confirmed: true }],
      },
      isLoading: false,
      isError: false,
    });
    rerender(<ApplicationDetailPage />);

    // The row is settled — no more provisional actions …
    expect(
      screen.queryByRole('button', { name: 'applications.detail.timeline.acceptAria' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'applications.detail.timeline.rejectAria' })
    ).not.toBeInTheDocument();
    // … and nothing on the page claims a revert happened.
    expect(screen.queryByText(/revert/i)).not.toBeInTheDocument();
    expect(
      screen.queryByText('applications.detail.timeline.correctionBadge')
    ).not.toBeInTheDocument();
  });

  it('renders a reversal row (source: email_reject) as a correction, never the raw backend note', () => {
    renderTimeline([
      makeEvent({
        at: 1000,
        fromStatus: 'applied',
        toStatus: 'interviewing',
        source: 'email',
        confirmed: true,
      }),
      makeEvent({
        at: 2000,
        fromStatus: 'interviewing',
        toStatus: 'applied',
        source: 'email_reject',
        confirmed: true,
        note: 'reverted: email-derived status change rejected by the user',
      }),
    ]);

    expect(screen.getByText('applications.detail.timeline.correctionBadge')).toBeInTheDocument();
    // The Rust reversal note is a fixed, non-localized English string — it
    // must never leak into the UI verbatim.
    expect(
      screen.queryByText('reverted: email-derived status change rejected by the user')
    ).not.toBeInTheDocument();
  });

  // `acceptStatusEvent` is ONE `useMutation()` instance shared by every row,
  // so `.variables`/`.isPending` reflect only the MOST RECENT `mutate()`
  // call — no expression over them can represent two rows genuinely
  // overlapping. This drives the actual overlap (start A, start B WHILE A is
  // still open, neither auto-resolving) rather than presetting a static
  // mock flag — a static fixture can't catch a timeline defect: the bug was
  // that starting B silently un-pends A, which only shows up if A is
  // demonstrably still in flight when B starts.
  it('keeps row A pending while row B starts a separate accept mid-flight — a shared mutation observer cannot represent two concurrent rows', async () => {
    const user = userEvent.setup();
    const rowA = makeEvent({
      eventId: 601,
      at: 1000,
      fromStatus: 'saved',
      toStatus: 'applied',
      source: 'email',
      confirmed: false,
    });
    const rowB = makeEvent({
      eventId: 602,
      at: 2000,
      fromStatus: 'applied',
      toStatus: 'rejected',
      source: 'email',
      confirmed: false,
    });

    // Neither call resolves on its own — the test decides exactly when EACH
    // one settles, independently, so the two mutations genuinely overlap
    // instead of one finishing before the other starts.
    const settleCallbacks: Array<() => void> = [];
    mockAcceptStatusEventMutate.mockImplementation(
      (_vars: unknown, options?: StatusEventMutateOptions) => {
        settleCallbacks.push(() => options?.onSettled?.());
      }
    );

    renderTimeline([rowA, rowB], { id: 'app-overlapping-accept' });

    // orderedEvents sorts newest-first, so the DOM order is [rowB, rowA].
    const acceptButtons = screen.getAllByRole('button', {
      name: 'applications.detail.timeline.acceptAria',
    });
    expect(acceptButtons).toHaveLength(2);
    const [bAccept, aAccept] = acceptButtons;
    if (!aAccept || !bAccept) throw new Error('expected two Accept buttons');

    // Start row A's accept — still in flight (the mock never auto-resolves).
    await user.click(aAccept);
    expect(aAccept).toBeDisabled();

    // Start row B's accept WHILE row A is still open.
    await user.click(bAccept);
    expect(settleCallbacks).toHaveLength(2);

    // Row A must STILL read pending — its own request hasn't settled, even
    // though B is now the most recent call on the SAME shared
    // `acceptStatusEvent` observer. Before the fix (gating on
    // `.variables?.eventId`, the last `mutate()` payload), starting B here
    // flips `.variables.eventId` to B's id and this assertion fails — A's
    // spinner disappears and both its buttons re-enable mid-write.
    expect(aAccept).toBeDisabled();
    expect(bAccept).toBeDisabled();

    // Settle ONLY row B's request.
    act(() => settleCallbacks[1]?.());
    expect(bAccept).not.toBeDisabled();
    // Row A is untouched by B settling — still its own in-flight request.
    expect(aAccept).toBeDisabled();

    // Settle row A's request too.
    act(() => settleCallbacks[0]?.());
    expect(aAccept).not.toBeDisabled();
  });

  // `applications_accept_status_event`/`applications_reject_status_event`
  // return `Value`, not `Result` — a backend failure resolves as `{ error }`
  // and `invoke` FULFILS, so `onError` never fires for it. The handler must
  // check `data.error` before showing the success toast, same as the
  // contact-write handlers elsewhere in this component.
  it('accept does NOT show the success toast when the backend resolves with an error', async () => {
    mockAcceptStatusEventMutate.mockImplementationOnce((_vars: unknown, options) => {
      options?.onSuccess?.({ error: 'db busy' });
    });
    renderTimeline([makeEvent({ source: 'email', confirmed: false })], {
      id: 'app-accept-backend-error',
    });

    await press('applications.detail.timeline.acceptAria');

    expect(mockNotify.success).not.toHaveBeenCalled();
    expect(mockNotify.error).toHaveBeenCalledWith({
      message: 'applications.detail.timeline.acceptError',
    });
  });

  it('reject does NOT show the success toast when the backend resolves with an error', async () => {
    mockRejectStatusEventMutate.mockImplementationOnce((_vars: unknown, options) => {
      options?.onSuccess?.({ error: 'db busy' });
    });
    renderTimeline([makeEvent({ source: 'email', confirmed: false })], {
      id: 'app-reject-backend-error',
    });

    await press('applications.detail.timeline.rejectAria');

    expect(mockNotify.success).not.toHaveBeenCalled();
    expect(mockNotify.error).toHaveBeenCalledWith({
      message: 'applications.detail.timeline.rejectError',
    });
  });
});
