/**
 * ReferralList — unit tests (F3a).
 *
 * Covers:
 *  - Status change upserts the FULL contact + new status (a partial { id, status }
 *    would blank the row — the backend overwrites every column by id).
 *  - Delete asks for confirmation first, then calls remove with the contact's id.
 *  - Nothing renders when the contacts array is empty.
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { ReferralContact, ReferralUpsertRequest } from '@ajh/shared/ipc';

// ── service mocks ─────────────────────────────────────────────────────────────

const mockUpsertMutate = vi.fn<(req: ReferralUpsertRequest) => void>();
const mockRemoveMutate = vi.fn<(id: string) => void>();

vi.mock('@/services', () => ({
  useUpsertReferral: () => ({ mutate: mockUpsertMutate, isPending: false }),
  useRemoveReferral: () => ({ mutate: mockRemoveMutate, isPending: false }),
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// SegmentedControl renders a set of radio buttons; no special stub needed.

// ── fixtures ──────────────────────────────────────────────────────────────────

import { ReferralList } from './ReferralList';

function makeContact(overrides: Partial<ReferralContact> = {}): ReferralContact {
  return {
    id: 'ref-1',
    jobUrl: 'https://acme.com/jobs/1',
    companyName: 'Acme',
    personName: 'Jane Smith',
    personRole: 'EM',
    channel: 'linkedin_message',
    status: 'draft',
    createdAt: 1_000,
    updatedAt: 1_000,
    ...overrides,
  };
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('ReferralList — empty state', () => {
  it('renders nothing when contacts array is empty', () => {
    const { container } = render(<ReferralList contacts={[]} />);
    expect(container.firstChild).toBeNull();
  });
});

describe('ReferralList — contact display', () => {
  it('renders the person name and company', () => {
    render(<ReferralList contacts={[makeContact()]} />);
    expect(screen.getByText('Jane Smith')).toBeInTheDocument();
    expect(screen.getByText(/Acme/)).toBeInTheDocument();
  });
});

describe('ReferralList — status change', () => {
  it('clicking "sent" upserts the full contact with status sent', () => {
    render(<ReferralList contacts={[makeContact({ id: 'ref-42', status: 'draft' })]} />);

    // SegmentedControl renders each option as a radio button whose accessible
    // name is the translated key — we use the raw key (t is identity here).
    const sentRadio = screen.getByRole('radio', {
      name: 'autopilot.referral.status.sent',
    });
    fireEvent.click(sentRadio);

    expect(mockUpsertMutate).toHaveBeenCalledTimes(1);
    // The whole contact is re-sent (not just { id, status }) so the full-row
    // overwrite preserves personName/company/role/channel/drafts.
    expect(mockUpsertMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'ref-42',
        personName: 'Jane Smith',
        companyName: 'Acme',
        personRole: 'EM',
        channel: 'linkedin_message',
        status: 'sent',
      })
    );
  });

  it('clicking "replied" upserts the full contact with status replied', () => {
    render(<ReferralList contacts={[makeContact({ id: 'ref-7', status: 'sent' })]} />);

    fireEvent.click(screen.getByRole('radio', { name: 'autopilot.referral.status.replied' }));

    expect(mockUpsertMutate).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'ref-7', personName: 'Jane Smith', status: 'replied' })
    );
  });
});

describe('ReferralList — delete', () => {
  it('confirming the delete dialog calls remove.mutate with the contact id', () => {
    render(<ReferralList contacts={[makeContact({ id: 'ref-99' })]} />);

    // The row trash button only opens the confirm dialog — no mutation yet.
    fireEvent.click(screen.getByRole('button', { name: 'autopilot.referral.delete' }));
    expect(mockRemoveMutate).not.toHaveBeenCalled();

    // The dialog's confirm button shares the `delete` label; it's the last one
    // in the document once the modal is mounted.
    const confirmButton = screen
      .getAllByRole('button', { name: 'autopilot.referral.delete' })
      .at(-1);
    if (!confirmButton) throw new Error('confirm button not found');
    fireEvent.click(confirmButton);

    expect(mockRemoveMutate).toHaveBeenCalledTimes(1);
    expect(mockRemoveMutate).toHaveBeenCalledWith('ref-99');
  });

  it('each contact row has its own delete button', () => {
    render(
      <ReferralList
        contacts={[
          makeContact({ id: 'ref-1', personName: 'Alice' }),
          makeContact({ id: 'ref-2', personName: 'Bob' }),
        ]}
      />
    );

    const deleteButtons = screen.getAllByRole('button', {
      name: 'autopilot.referral.delete',
    });
    expect(deleteButtons).toHaveLength(2);
  });
});
