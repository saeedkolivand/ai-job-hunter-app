/**
 * ReferralUpsertSchema parse-contract tests (F3a).
 *
 * Validates the Zod schema gate that the Rust command and the modal's upsert
 * call both rely on. These tests are the "test the allowlist" complement that
 * the Rust allowlist-skip comment defers to.
 *
 * Covers:
 *  - Accepts the exact payload shapes the modal builds for each channel.
 *  - Rejects an invalid `channel` value.
 *  - Rejects an invalid `status` value.
 *  - Applies defaults for `channel` and `status` when omitted.
 */
import { describe, expect, it } from 'vitest';

import { ReferralUpsertSchema } from './index';

// ─── Valid payload shapes — one per channel ───────────────────────────────────

describe('ReferralUpsertSchema — accepts valid payloads', () => {
  it('accepts the linkedin_message payload shape the modal builds', () => {
    const result = ReferralUpsertSchema.safeParse({
      jobUrl: 'https://acme.com/jobs/1',
      companyName: 'Acme',
      personName: 'Bob Chen',
      channel: 'linkedin_message',
      messageDraft: 'Hi Bob, can you refer me?',
      status: 'draft',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.channel).toBe('linkedin_message');
      expect(result.data.messageDraft).toBe('Hi Bob, can you refer me?');
      expect(result.data.status).toBe('draft');
    }
  });

  it('accepts the email payload shape with emailDraft', () => {
    const result = ReferralUpsertSchema.safeParse({
      jobUrl: 'https://acme.com/jobs/1',
      companyName: 'Acme',
      personName: 'Bob Chen',
      channel: 'email',
      emailDraft: 'Subject: Referral request\n\nHi Bob,',
      status: 'draft',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.channel).toBe('email');
      expect(result.data.emailDraft).toBe('Subject: Referral request\n\nHi Bob,');
    }
  });

  it('accepts the connection_note payload shape with inviteNoteDraft', () => {
    const result = ReferralUpsertSchema.safeParse({
      jobUrl: 'https://acme.com/jobs/1',
      companyName: 'Acme',
      personName: 'Bob Chen',
      channel: 'connection_note',
      inviteNoteDraft: 'Hi, I am applying to Acme and would love a referral.',
      status: 'draft',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.channel).toBe('connection_note');
      expect(result.data.inviteNoteDraft).toBe(
        'Hi, I am applying to Acme and would love a referral.'
      );
    }
  });

  it('accepts optional fields — personRole, linkedinUrl, notes — alongside a channel payload', () => {
    const result = ReferralUpsertSchema.safeParse({
      jobUrl: 'https://acme.com/jobs/2',
      companyName: 'Globex',
      personName: 'Alice Li',
      personRole: 'Engineering Director',
      linkedinUrl: 'https://linkedin.com/in/alice',
      notes: 'Met at a conference.',
      channel: 'email',
      emailDraft: 'Subject: Hi\n\nBody.',
      status: 'sent',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.personRole).toBe('Engineering Director');
      expect(result.data.status).toBe('sent');
    }
  });

  it('accepts an upsert that includes an id (update path)', () => {
    const result = ReferralUpsertSchema.safeParse({
      id: 'referral-abc-123',
      channel: 'linkedin_message',
      messageDraft: 'Updated draft',
      status: 'replied',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.id).toBe('referral-abc-123');
      expect(result.data.status).toBe('replied');
    }
  });
});

// ─── Defaults ─────────────────────────────────────────────────────────────────

describe('ReferralUpsertSchema — applies defaults', () => {
  it('defaults channel to "email" and status to "draft" when both are omitted', () => {
    const result = ReferralUpsertSchema.safeParse({});

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.channel).toBe('email');
      expect(result.data.status).toBe('draft');
    }
  });
});

// ─── Rejection — invalid enum values ─────────────────────────────────────────

describe('ReferralUpsertSchema — rejects invalid enum values', () => {
  it('rejects an invalid channel value (e.g. "sms")', () => {
    const result = ReferralUpsertSchema.safeParse({
      channel: 'sms',
    });

    expect(result.success).toBe(false);
  });

  it('rejects an invalid status value (e.g. "archived")', () => {
    const result = ReferralUpsertSchema.safeParse({
      status: 'archived',
    });

    expect(result.success).toBe(false);
  });

  it('rejects channel: null', () => {
    const result = ReferralUpsertSchema.safeParse({ channel: null });
    expect(result.success).toBe(false);
  });

  it('rejects status: null', () => {
    const result = ReferralUpsertSchema.safeParse({ status: null });
    expect(result.success).toBe(false);
  });
});
