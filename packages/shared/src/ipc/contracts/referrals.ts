/** The channel the user plans to reach the referral contact through. */
export type ReferralChannel = 'email' | 'linkedin_message' | 'connection_note';

/** Where a referral ask stands in the user's manual outreach flow. */
export type ReferralStatus = 'draft' | 'sent' | 'replied';

/**
 * A locally-stored "referral contact": a person the user wants to ask for a
 * referral at a target company. Every detail is entered MANUALLY by the user —
 * there is no LinkedIn scraping or profile fetch. `linkedinUrl` is just an
 * optional free-text field the user pastes in.
 */
export interface ReferralContact {
  id: string;
  /** The job this referral targets — links the contact to an autopilot found job. */
  jobUrl: string;
  companyName: string;
  personName: string;
  /** The person's role/title, if the user noted it. */
  personRole?: string;
  /** Manual free text — never fetched or scraped. */
  linkedinUrl?: string;
  /** A drafted referral email, if any. */
  emailDraft?: string;
  /** A drafted LinkedIn message, if any. */
  messageDraft?: string;
  /** A drafted connection-request note, if any. */
  inviteNoteDraft?: string;
  channel: ReferralChannel;
  status: ReferralStatus;
  /** Free-form notes about the contact. */
  notes?: string;
  createdAt: number;
  updatedAt: number;
}

/**
 * Create or update a referral contact in one call. An absent `id` inserts a
 * fresh row (the store assigns the id, `createdAt`, and `updatedAt`); a present
 * `id` overwrites that row and bumps `updatedAt`.
 */
export interface ReferralUpsertRequest {
  /** Absent → insert; present → overwrite the row with this id. */
  id?: string;
  jobUrl?: string;
  companyName?: string;
  personName?: string;
  personRole?: string;
  /** Manual free text — never fetched or scraped. */
  linkedinUrl?: string;
  emailDraft?: string;
  messageDraft?: string;
  inviteNoteDraft?: string;
  channel?: ReferralChannel;
  status?: ReferralStatus;
  notes?: string;
}

export interface ReferralsContract {
  /** All referral contacts, optionally filtered to one job's `jobUrl`. */
  list(jobUrl?: string): Promise<ReferralContact[]>;
  /** Create or update a contact; resolves to the stored record. */
  upsert(req: ReferralUpsertRequest): Promise<ReferralContact>;
  remove(id: string): Promise<void>;
}

export const REFERRALS_CHANNELS = {
  list: 'referrals:list',
  upsert: 'referrals:upsert',
  remove: 'referrals:remove',
} as const;
