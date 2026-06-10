/**
 * ReferralModal — save/upsert integration tests (F3a).
 *
 * Covers:
 *  - connection_note >300 chars: Save and Copy buttons are disabled (overLimit).
 *  - connection_note ≤300 chars: Save is enabled and persists via upsert.mutate.
 *  - Save calls upsert.mutate with the expected payload shape
 *    (jobUrl, personName, channel, the correct draft field, status="draft").
 *
 * Heavy UI pieces (ModelSelector, ModalShell focus trap, ReferralList, the
 * streaming draft hook) are replaced with lightweight stubs so this test stays
 * fast and deterministic. The real component logic (overLimit computation, canSave
 * guard, payload mapping) is still exercised through the real ReferralModal code.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import type { AutopilotFoundJob } from '@ajh/shared';
import type { ReferralContact, ReferralUpsertRequest } from '@ajh/shared/ipc';
import type * as AjhUi from '@ajh/ui';

// ── service mocks ─────────────────────────────────────────────────────────────

const mockUpsertMutate =
  vi.fn<(req: ReferralUpsertRequest, opts?: { onSuccess?: () => void }) => void>();

// Mutable per-test list so a test can seed an existing contact and exercise the
// dedup branch in save() (re-saving the same person carries that row's id).
let stubbedContacts: ReferralContact[] = [];

vi.mock('@/services', () => ({
  useReferrals: () => ({ data: stubbedContacts }),
  useUpsertReferral: () => ({ mutate: mockUpsertMutate, isPending: false }),
  // useRemoveReferral is consumed by ReferralList (which is stubbed below), so
  // it only needs to exist for the service barrel import to resolve.
  useRemoveReferral: () => ({ mutate: vi.fn(), isPending: false }),
}));

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@/lib/i18n', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── AI-capability hooks + ModelSelector ──────────────────────────────────────
// Stub the whole ModelSelector component so we don't need the full provider tree.

vi.mock('@/components/ui/ModelSelector', () => ({
  ModelSelector: () => null,
  useCanUseAI: () => ({ canUse: true, reason: null }),
  useSelectedModel: () => 'llama3',
}));

// ── useReferralDraft — stub to control `draft` and `generate` deterministically

// We need to override the draft value per test. Use a module-level ref so each
// it() can set its own value before rendering.
let stubbedDraft = '';
const mockGenerate = vi.fn<() => Promise<void>>(async () => {});

vi.mock('./useReferralDraft', () => ({
  useReferralDraft: () => ({
    draft: stubbedDraft,
    generating: false,
    error: null,
    generate: mockGenerate,
    abort: vi.fn(),
    canGenerate: true,
    // save()'s onSuccess now calls reset() (the add-another flow) — stub it so the
    // success path doesn't throw on an undefined.
    reset: vi.fn(),
  }),
}));

// ── ReferralList stub ─────────────────────────────────────────────────────────
// The list is tested in its own file; stub it here to avoid double-rendering issues.

vi.mock('./ReferralList', () => ({
  ReferralList: () => null,
}));

// ── ModalShell stub ───────────────────────────────────────────────────────────
// ModalShell manages focus traps and portals — render children directly.

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    ModalShell: ({ children }: { children: React.ReactNode }) => (
      <div data-testid="modal-shell">{children}</div>
    ),
  };
});

// ── component under test ──────────────────────────────────────────────────────

import { ReferralModal } from './index';

beforeEach(() => {
  stubbedDraft = '';
  stubbedContacts = [];
  mockUpsertMutate.mockClear();
  mockGenerate.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
});

// ── fixtures ──────────────────────────────────────────────────────────────────

const JOB: AutopilotFoundJob = {
  title: 'Senior Engineer',
  company: 'Acme',
  url: 'https://acme.com/jobs/1',
  foundAt: 1_000,
};

const RESUME = 'Jane Doe\nSenior Engineer with 8 years of experience.';

function renderModal(jobOverrides: Partial<AutopilotFoundJob> = {}) {
  const onClose = vi.fn();
  render(<ReferralModal job={{ ...JOB, ...jobOverrides }} resume={RESUME} onClose={onClose} />);
  return { onClose };
}

/** Fill the person-name input so canSave can become true. */
function fillPersonName(name = 'Bob Chen') {
  const input = screen.getByPlaceholderText('autopilot.referral.personNamePlaceholder');
  fireEvent.change(input, { target: { value: name } });
}

/** Switch to the given channel via its SegmentedControl radio. */
function switchChannel(channelKey: string) {
  fireEvent.click(screen.getByRole('radio', { name: `autopilot.referral.channel.${channelKey}` }));
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('ReferralModal — connection_note overLimit disables Save + Copy', () => {
  it('Save button is disabled when connection_note draft exceeds 300 chars', () => {
    stubbedDraft = 'A'.repeat(301);

    renderModal();
    fillPersonName();
    switchChannel('connection_note');

    const saveBtn = screen.getByRole('button', { name: /autopilot\.referral\.save/i });
    expect(saveBtn).toBeDisabled();
  });

  it('Copy button is disabled when connection_note draft exceeds 300 chars', () => {
    stubbedDraft = 'A'.repeat(301);

    renderModal();
    fillPersonName();
    switchChannel('connection_note');

    const copyBtn = screen.getByRole('button', { name: /autopilot\.referral\.copy/i });
    expect(copyBtn).toBeDisabled();
  });

  it('Save button is NOT disabled when connection_note draft is exactly 300 chars', () => {
    stubbedDraft = 'A'.repeat(300);

    renderModal();
    fillPersonName();
    switchChannel('connection_note');

    const saveBtn = screen.getByRole('button', { name: /autopilot\.referral\.save/i });
    expect(saveBtn).not.toBeDisabled();
  });
});

describe('ReferralModal — Save persists via upsert with correct payload', () => {
  it('Save for linkedin_message calls upsert.mutate with messageDraft + status=draft', () => {
    stubbedDraft = 'Hi Bob, can you refer me?';

    renderModal();
    fillPersonName('Bob Chen');
    // Default channel is linkedin_message — no explicit switch needed.

    const saveBtn = screen.getByRole('button', { name: /autopilot\.referral\.save/i });
    act(() => {
      fireEvent.click(saveBtn);
    });

    expect(mockUpsertMutate).toHaveBeenCalledTimes(1);
    expect(mockUpsertMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://acme.com/jobs/1',
        companyName: 'Acme',
        personName: 'Bob Chen',
        channel: 'linkedin_message',
        messageDraft: 'Hi Bob, can you refer me?',
        status: 'draft',
      }),
      expect.any(Object)
    );
  });

  it('Save for email uses emailDraft field', () => {
    stubbedDraft = 'Subject: Referral request\n\nHi Bob,';

    renderModal();
    fillPersonName('Bob Chen');
    switchChannel('email');

    act(() => {
      fireEvent.click(screen.getByRole('button', { name: /autopilot\.referral\.save/i }));
    });

    expect(mockUpsertMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        channel: 'email',
        emailDraft: 'Subject: Referral request\n\nHi Bob,',
        status: 'draft',
      }),
      expect.any(Object)
    );
  });

  it('Save for connection_note (≤300) uses inviteNoteDraft field', () => {
    stubbedDraft = 'Hi, I am applying to Acme and would love a referral.';

    renderModal();
    fillPersonName('Bob Chen');
    switchChannel('connection_note');

    act(() => {
      fireEvent.click(screen.getByRole('button', { name: /autopilot\.referral\.save/i }));
    });

    expect(mockUpsertMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        channel: 'connection_note',
        inviteNoteDraft: 'Hi, I am applying to Acme and would love a referral.',
        status: 'draft',
      }),
      expect.any(Object)
    );
  });

  it('Save for an EXISTING person (same name, this job) carries their id + other drafts', () => {
    // Dedup branch: a contact with the same name already exists for this job, so the
    // save must update that row (id present) and preserve the other channels' drafts
    // instead of inserting a duplicate.
    stubbedContacts = [
      {
        id: 'ref-1',
        jobUrl: 'https://acme.com/jobs/1',
        companyName: 'Acme',
        personName: 'Bob Chen',
        personRole: undefined,
        linkedinUrl: undefined,
        emailDraft: 'old email draft',
        messageDraft: 'old message draft',
        inviteNoteDraft: undefined,
        channel: 'email',
        status: 'sent',
        notes: undefined,
        createdAt: 1_000,
      } as ReferralContact,
    ];
    stubbedDraft = 'Hi Bob, can you refer me?';

    renderModal();
    // Match case-insensitively — lower-case input must still hit the existing row.
    fillPersonName('bob chen');
    // Default channel is linkedin_message → messageDraft is the field being set.

    act(() => {
      fireEvent.click(screen.getByRole('button', { name: /autopilot\.referral\.save/i }));
    });

    expect(mockUpsertMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'ref-1',
        // Other channels' drafts are carried so the full-row overwrite doesn't blank them.
        emailDraft: 'old email draft',
        inviteNoteDraft: undefined,
        // The current channel's draft is set last and wins.
        messageDraft: 'Hi Bob, can you refer me?',
        personName: 'bob chen',
        channel: 'linkedin_message',
        status: 'draft',
      }),
      expect.any(Object)
    );
  });

  it('Save button is disabled when personName is blank', () => {
    stubbedDraft = 'Hi, can you refer me?';

    renderModal();
    // Intentionally do NOT fill the person name — canSave requires personName.

    const saveBtn = screen.getByRole('button', { name: /autopilot\.referral\.save/i });
    expect(saveBtn).toBeDisabled();

    act(() => {
      fireEvent.click(saveBtn);
    });

    // Even if the click fires (disabled buttons still receive events in JSDOM),
    // the save() guard checks personName and returns early.
    expect(mockUpsertMutate).not.toHaveBeenCalled();
  });

  it('Save button is absent when draft is empty (conditional render)', () => {
    // stubbedDraft = '' (reset in beforeEach)
    renderModal();
    fillPersonName('Bob Chen');

    // The entire draft output section — including Save — is conditionally
    // rendered only when gen.draft is non-empty, so the button must not exist.
    expect(screen.queryByRole('button', { name: /autopilot\.referral\.save/i })).toBeNull();
    expect(mockUpsertMutate).not.toHaveBeenCalled();
  });
});

describe('ReferralModal — onSuccess saved flash', () => {
  it('Save button shows saved label immediately after upsert.mutate calls onSuccess', () => {
    stubbedDraft = 'Hi Bob, can you refer me?';

    // Make mutate invoke its onSuccess callback synchronously so we can assert
    // the setSaved(true) effect without fake timers.
    mockUpsertMutate.mockImplementationOnce(
      (_req: ReferralUpsertRequest, opts?: { onSuccess?: () => void }) => {
        opts?.onSuccess?.();
      }
    );

    renderModal();
    fillPersonName('Bob Chen');

    act(() => {
      fireEvent.click(screen.getByRole('button', { name: /autopilot\.referral\.save/i }));
    });

    // setSaved(true) must have fired — the button now shows the "saved" key.
    expect(screen.getByRole('button', { name: /autopilot\.referral\.saved/i })).toBeInTheDocument();
  });
});

describe('ReferralModal — over-limit counter text', () => {
  it('shows X/300 counter text when channel is connection_note', () => {
    stubbedDraft = 'short note';

    renderModal();
    switchChannel('connection_note');

    // The counter renders as "{len}/300" — check the "/300" part.
    expect(screen.getByText(/\/300/)).toBeInTheDocument();
  });

  it('shows over-limit text when connection_note draft exceeds 300 chars', () => {
    stubbedDraft = 'A'.repeat(301);

    renderModal();
    switchChannel('connection_note');

    expect(screen.getByText(/autopilot\.referral\.overLimit/)).toBeInTheDocument();
  });
});
