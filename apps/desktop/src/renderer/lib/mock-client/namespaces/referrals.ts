import type { ReferralContact, ReferralUpsertRequest } from '@ajh/shared';

import type { AppClient } from '../../app-client';

/**
 * In-memory referral store so the renderer/tests can exercise list/upsert/remove
 * offline without a backend. Scoped per call so each mock client starts empty.
 */
export function createMockReferrals(): AppClient['referrals'] {
  const referralRows: ReferralContact[] = [];
  return {
    list: async (jobUrl?: string) =>
      jobUrl ? referralRows.filter((r) => r.jobUrl === jobUrl) : [...referralRows],
    upsert: async (req: ReferralUpsertRequest) => {
      const now = Date.now();
      const existing = req.id ? referralRows.find((r) => r.id === req.id) : undefined;
      const record: ReferralContact = {
        id: existing?.id ?? req.id ?? `ref-${now}-${Math.random().toString(36).slice(2, 10)}`,
        jobUrl: req.jobUrl ?? '',
        companyName: req.companyName ?? '',
        personName: req.personName ?? '',
        personRole: req.personRole ?? '',
        linkedinUrl: req.linkedinUrl ?? '',
        emailDraft: req.emailDraft ?? '',
        messageDraft: req.messageDraft ?? '',
        inviteNoteDraft: req.inviteNoteDraft ?? '',
        channel: req.channel ?? 'email',
        status: req.status ?? 'draft',
        notes: req.notes ?? '',
        createdAt: existing?.createdAt ?? now,
        updatedAt: now,
      };
      if (existing) {
        referralRows.splice(referralRows.indexOf(existing), 1, record);
      } else {
        referralRows.push(record);
      }
      return record;
    },
    remove: async (id: string) => {
      const i = referralRows.findIndex((r) => r.id === id);
      if (i >= 0) referralRows.splice(i, 1);
    },
  };
}
