import { z } from 'zod';

// Manual referral helper — a locally-stored "referral contact" the user wants to
// ask for a referral at a target company. Create OR update in one call: an absent
// `id` inserts a fresh row, a present `id` overwrites that row. Every person
// detail is entered MANUALLY by the user — there is no LinkedIn scraping or
// profile fetch; `linkedinUrl` is just an optional free-text field.
export const ReferralUpsertSchema = z.object({
  // Absent → insert a new contact; present → overwrite the row with this id.
  id: z.string().optional(),
  // The job this referral targets (links to the autopilot found job; indexed).
  jobUrl: z.string().default(''),
  companyName: z.string().default(''),
  personName: z.string().default(''),
  personRole: z.string().optional(),
  // Manual free text — NOT fetched/scraped.
  linkedinUrl: z.string().optional(),
  emailDraft: z.string().optional(),
  messageDraft: z.string().optional(),
  inviteNoteDraft: z.string().optional(),
  channel: z.enum(['email', 'linkedin_message', 'connection_note']).default('email'),
  status: z.enum(['draft', 'sent', 'replied']).default('draft'),
  notes: z.string().optional(),
});
// Note: the `ReferralUpsertRequest` type is declared in the referrals IPC
// contract (single source for that name); this schema validates the same shape.

// ─── Application tracking schemas (ADR 0001) ───────────────────────────────────

// Manual create / Jobs-page Save. `applications_track` marks it `applied`;
// `applications_save_from_posting` keeps it `saved`. All fields optional — a
// hand-tracked application may have no link yet.
export const ApplicationTrackSchema = z.object({
  // Optional job link. Empty → a link-less pursuit (its own Application).
  jobUrl: z.string().optional(),
  board: z.string().optional(),
  company: z.string().optional(),
  title: z.string().optional(),
  candidate: z.string().optional(),
  // Job description captured at save time (e.g. an aggregator posting whose URL is
  // a redirect that can't be re-resolved). Carried so tailoring has the ad text
  // without a second fetch. Same byte-bound refine as ApplicationUpdateSchema.
  jobDescription: z
    .string()
    .refine((v) => new TextEncoder().encode(v).length <= 200_000, {
      message: 'jobDescription must be at most 200000 bytes',
    })
    .optional(),
  // Scraped salary (Adzuna only, today) — grounds the salary application answer.
  // Absent/unknown when the board didn't report one.
  salaryMin: z.number().optional(),
  salaryMax: z.number().optional(),
  salaryCurrency: z.string().optional(),
});
export type ApplicationTrackRequest = z.infer<typeof ApplicationTrackSchema>;

// Patch the user-editable tracking fields of an existing Application. Each field
// is optional; an absent field is left unchanged. `nextActionAt` is nullable to
// allow explicitly clearing the reminder.
export const ApplicationUpdateSchema = z.object({
  id: z.string().min(1),
  notes: z.string().optional(),
  // Non-negative: the server-side guard (`parse_next_action_at`) rejects a
  // negative epoch-ms, so the wire contract mirrors it rather than silently
  // clearing the reminder on a bad value.
  // Clearing the reminder (explicit null) or moving it to a new date also
  // resets the backend's follow-up-notification marker, so the new due date
  // notifies once. Setting the SAME value again is not a reschedule.
  nextActionAt: z.number().int().min(0).nullable().optional(),
  comp: z.string().optional(),
  // The canonical primary contact for the application (recruiter / hiring
  // manager / apply-by-email recipient — one person, one pair). Server-side
  // BOTH inbound names go through the same trim + byte-cap + address-format
  // guards, so the caps here match the deprecated aliases below exactly.
  //
  // Byte-length (not `.max()`'s char-count), matching the Rust guards and the
  // `jobDescription` precedent below: a 200-CHARACTER CJK name is 600 bytes, so
  // a char cap passed here and then failed server-side with an error the user
  // could do nothing about.
  contactName: z
    .string()
    .trim()
    .refine((v) => new TextEncoder().encode(v).length <= 200, {
      message: 'contactName must be at most 200 bytes',
    })
    .optional(),
  contactEmail: z
    .string()
    .trim()
    .refine((v) => new TextEncoder().encode(v).length <= 254, {
      message: 'contactEmail must be at most 254 bytes',
    })
    .optional(),
  // The imported/pasted job description, persisted onto the Application so a JD
  // captured from the browser DOM survives to tailoring. Capped to a sane bound
  // so a pathological paste can't bloat the row. Byte-length (not char-count) so
  // it matches the Rust store's 200_000-BYTE limit — multi-byte UTF-8 otherwise
  // passes validation then gets silently truncated.
  // ponytail: 200 KB ceiling matches the 8 MB-frame era; raise if real JDs exceed it.
  jobDescription: z
    .string()
    .refine((v) => new TextEncoder().encode(v).length <= 200_000, {
      message: 'jobDescription must be at most 200000 bytes',
    })
    .optional(),
  jobSummary: z.string().max(50_000).optional(),
  // DEPRECATED aliases of contactName/contactEmail, still accepted so existing
  // callers (the apply-by-email tab, the extension) keep working: a write under
  // either name lands in the SAME storage, and both names come back populated
  // with that one value. Sending both in one patch → the canonical one wins.
  // Byte-capped identically to the canonical pair above — they hit one column,
  // so a laxer alias would just be a way around the canonical bound.
  recipientName: z
    .string()
    .trim()
    .refine((v) => new TextEncoder().encode(v).length <= 200, {
      message: 'recipientName must be at most 200 bytes',
    })
    .optional(),
  recipientEmail: z
    .string()
    .trim()
    .refine((v) => new TextEncoder().encode(v).length <= 254, {
      message: 'recipientEmail must be at most 254 bytes',
    })
    .optional(),
});
export type ApplicationUpdateRequest = z.infer<typeof ApplicationUpdateSchema>;
