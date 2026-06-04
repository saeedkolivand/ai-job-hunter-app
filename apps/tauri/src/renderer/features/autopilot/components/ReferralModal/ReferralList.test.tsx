/**
 * ReferralList — unit tests (F3a).
 *
 * Covers:
 *  - Status SegmentedControl change calls upsert with { id, status }.
 *  - Delete button calls remove with the contact's id.
 *  - Nothing renders when the contacts array is empty.
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { ReferralContact } from '@ajh/shared/ipc';

// ── service mocks ─────────────────────────────────────────────────────────────

const mockUpsertMutate = vi.fn<(req: { id: string; status: string }) => void>();
const mockRemoveMutate = vi.fn<(id: string) => void>();

vi.mock('@/services', () => ({
  useUpsertReferral: () => ({ mutate: mockUpsertMutate, isPending: false }),
  useRemoveReferral: () => ({ mutate: mockRemoveMutate, isPending: false }),
}));

vi.mock('@/lib/i18n', () => ({
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
  it('clicking "sent" calls upsert.mutate with { id, status: "sent" }', () => {
    render(<ReferralList contacts={[makeContact({ id: 'ref-42', status: 'draft' })]} />);

    // SegmentedControl renders each option as a radio button whose accessible
    // name is the translated key — we use the raw key (t is identity here).
    const sentRadio = screen.getByRole('radio', {
      name: 'autopilot.referral.status.sent',
    });
    fireEvent.click(sentRadio);

    expect(mockUpsertMutate).toHaveBeenCalledTimes(1);
    expect(mockUpsertMutate).toHaveBeenCalledWith({ id: 'ref-42', status: 'sent' });
  });

  it('clicking "replied" calls upsert.mutate with { id, status: "replied" }', () => {
    render(<ReferralList contacts={[makeContact({ id: 'ref-7', status: 'sent' })]} />);

    fireEvent.click(screen.getByRole('radio', { name: 'autopilot.referral.status.replied' }));

    expect(mockUpsertMutate).toHaveBeenCalledWith({ id: 'ref-7', status: 'replied' });
  });
});

describe('ReferralList — delete', () => {
  it('delete button calls remove.mutate with the contact id', () => {
    render(<ReferralList contacts={[makeContact({ id: 'ref-99' })]} />);

    fireEvent.click(screen.getByRole('button', { name: 'autopilot.referral.delete' }));

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
