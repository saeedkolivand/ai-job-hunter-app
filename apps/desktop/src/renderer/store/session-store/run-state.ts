import { create } from 'zustand';

import type { ReferralChannel } from '@ajh/shared/ipc';

/**
 * The Referral modal's most recent draft run (rule 16): the stream is
 * renderer-driven, so it keeps writing here after the modal unmounts and the
 * reopened modal for the same job (`jobUrl`) shows it in flight or finished.
 */
export interface ReferralDraftSlice {
  jobUrl: string;
  personName: string;
  personRole: string;
  channel: ReferralChannel;
  draft: string;
  generating: boolean;
  error: string | null;
}

export const REFERRAL_DRAFT_DEFAULTS: ReferralDraftSlice = {
  jobUrl: '',
  personName: '',
  personRole: '',
  channel: 'linkedin_message',
  draft: '',
  generating: false,
  error: null,
};

interface ReferralDraftStore {
  referralDraft: ReferralDraftSlice;
  setReferralDraft: (patch: Partial<ReferralDraftSlice>) => void;
  resetReferralDraft: () => void;
}

/** Memory-only, like the session store; split out to keep that file under the size cap. */
export const useReferralDraftStore = create<ReferralDraftStore>((set) => ({
  referralDraft: { ...REFERRAL_DRAFT_DEFAULTS },
  setReferralDraft: (patch) => set((s) => ({ referralDraft: { ...s.referralDraft, ...patch } })),
  resetReferralDraft: () => set({ referralDraft: { ...REFERRAL_DRAFT_DEFAULTS } }),
}));
