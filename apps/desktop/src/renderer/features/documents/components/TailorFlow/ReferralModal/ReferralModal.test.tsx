/**
 * ReferralModal — save/upsert integration tests (F3a).
 *
 * Covers:
 *  - connection_note >300 chars: Save and Copy buttons are disabled (overLimit).
 *  - connection_note ≤300 chars: Save is enabled and persists via upsert.mutate.
 *  - Save calls upsert.mutate with the expected payload shape
 *    (jobUrl, personName, channel, the correct draft field, status="draft").
 *
 * The Improve-with-AI affordance lives in `ReferralModal.improve.test.tsx`; shared
 * stubs in `modal.test-support.tsx`.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';

import type { ReferralContact, ReferralUpsertRequest } from '@ajh/shared/ipc';
import type * as AjhUi from '@ajh/ui';

import {
  clickSave,
  fillPersonName,
  getSaveBtn,
  renderModal,
  switchChannel,
} from './modal.test-helpers';
import { mockUpsertMutate, resetStub, stub } from './modal.test-support';

vi.mock('@/services', async () => (await import('./modal.test-support')).servicesModule);
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
vi.mock('@/components/ui/ModelSelector', async () => {
  return (await import('./modal.test-support')).modelSelectorModule;
});
vi.mock(
  './useReferralDraft',
  async () => (await import('./modal.test-support')).referralDraftModule
);
vi.mock('./ReferralList', async () => (await import('./modal.test-support')).referralListModule);
vi.mock('@ajh/ui', async (importOriginal) => {
  return (await import('./modal.test-support')).uiModule(await importOriginal<typeof AjhUi>());
});

beforeEach(resetStub);

afterEach(() => {
  vi.clearAllMocks();
});

// ── tests ─────────────────────────────────────────────────────────────────────

/** Renders with the stubbed `draft`, optionally typing a person name and picking a channel. */
function setup(
  draft: string,
  { name = 'Bob Chen', channel }: { name?: string | null; channel?: string } = {}
) {
  stub.draft = draft;
  renderModal();
  if (name) fillPersonName(name);
  if (channel) switchChannel(channel);
}

describe('ReferralModal — connection_note overLimit disables Save + Copy', () => {
  it.each([
    ['Save button is disabled when connection_note draft exceeds 300 chars', 301, 'save', true],
    ['Copy button is disabled when connection_note draft exceeds 300 chars', 301, 'copy', true],
    [
      'Save button is NOT disabled when connection_note draft is exactly 300 chars',
      300,
      'save',
      false,
    ],
  ] as const)('%s', (_name, length, button, disabled) => {
    setup('A'.repeat(length), { channel: 'connection_note' });

    const btn =
      button === 'save'
        ? getSaveBtn()
        : screen.getByRole('button', { name: /autopilot\.referral\.copy/i });
    if (disabled) expect(btn).toBeDisabled();
    else expect(btn).not.toBeDisabled();
  });
});

describe('ReferralModal — Save persists via upsert with correct payload', () => {
  // Default channel is linkedin_message — no explicit switch needed.
  it.each([
    [
      'Save for linkedin_message calls upsert.mutate with messageDraft + status=draft',
      undefined,
      'Hi Bob, can you refer me?',
      'linkedin_message',
      'messageDraft',
    ],
    [
      'Save for email uses emailDraft field',
      'email',
      'Subject: Referral request\n\nHi Bob,',
      'email',
      'emailDraft',
    ],
    [
      'Save for connection_note (≤300) uses inviteNoteDraft field',
      'connection_note',
      'Hi, I am applying to Acme and would love a referral.',
      'connection_note',
      'inviteNoteDraft',
    ],
  ])('%s', (_name, channelKey, draft, channel, field) => {
    setup(draft, { channel: channelKey });

    clickSave();

    expect(mockUpsertMutate).toHaveBeenCalledTimes(1);
    expect(mockUpsertMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://acme.com/jobs/1',
        companyName: 'Acme',
        personName: 'Bob Chen',
        channel,
        [field]: draft,
        status: 'draft',
      }),
      expect.any(Object)
    );
  });

  it('Save for an EXISTING person (same name, this job) carries their id + other drafts', () => {
    // Dedup branch: a contact with the same name already exists for this job, so the
    // save must update that row (id present) and preserve the other channels' drafts
    // instead of inserting a duplicate.
    stub.contacts = [
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
    // Match case-insensitively — lower-case input must still hit the existing row.
    // Default channel is linkedin_message → messageDraft is the field being set.
    setup('Hi Bob, can you refer me?', { name: 'bob chen' });

    clickSave();

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
    // Intentionally do NOT fill the person name — canSave requires personName.
    setup('Hi, can you refer me?', { name: null });

    expect(getSaveBtn()).toBeDisabled();

    clickSave();

    // Even if the click fires (disabled buttons still receive events in JSDOM),
    // the save() guard checks personName and returns early.
    expect(mockUpsertMutate).not.toHaveBeenCalled();
  });

  it('Save button is absent when draft is empty (conditional render)', () => {
    setup('');

    // The entire draft output section — including Save — is conditionally
    // rendered only when gen.draft is non-empty, so the button must not exist.
    expect(screen.queryByRole('button', { name: /autopilot\.referral\.save/i })).toBeNull();
    expect(mockUpsertMutate).not.toHaveBeenCalled();
  });
});

describe('ReferralModal — onSuccess saved flash', () => {
  it('Save button shows saved label immediately after upsert.mutate calls onSuccess', () => {
    // Make mutate invoke its onSuccess callback synchronously so we can assert
    // the setSaved(true) effect without fake timers.
    mockUpsertMutate.mockImplementationOnce(
      (_req: ReferralUpsertRequest, opts?: { onSuccess?: () => void }) => {
        opts?.onSuccess?.();
      }
    );
    setup('Hi Bob, can you refer me?');

    clickSave();

    // setSaved(true) must have fired — the button now shows the "saved" key.
    expect(screen.getByRole('button', { name: /autopilot\.referral\.saved/i })).toBeInTheDocument();
  });
});

describe('ReferralModal — over-limit counter text', () => {
  it.each([
    ['shows X/300 counter text when channel is connection_note', 'short note', /\/300/],
    [
      'shows over-limit text when connection_note draft exceeds 300 chars',
      'A'.repeat(301),
      /autopilot\.referral\.overLimit/,
    ],
  ])('%s', (_name, draft, text) => {
    setup(draft, { name: null, channel: 'connection_note' });

    // The counter renders as "{len}/300" — check the "/300" part.
    expect(screen.getByText(text)).toBeInTheDocument();
  });
});
