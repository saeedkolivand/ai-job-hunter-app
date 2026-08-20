import type { ApplicationTrackRequest, ApplicationUpdateRequest } from '../../schemas/index.js';
import type { Application, StatusEvent } from '../../types/index.js';

export type { ApplicationTrackRequest, ApplicationUpdateRequest };

/** The detail payload for one Application: the aggregate plus its status history. */
export interface ApplicationDetail {
  application: Application | null;
  events: StatusEvent[];
}

/** Result of a mutating command (matches the Rust `{ success } | { error }` shape). */
export interface ApplicationMutationResult {
  success?: boolean;
  error?: string;
}

/** Result of a create command. */
export interface ApplicationCreateResult {
  id?: string;
  success?: boolean;
  error?: string;
}

/**
 * Event payload emitted when an Application is created/changed out-of-band — e.g.
 * a job imported via the browser-extension bridge. Carries the affected id so
 * consumers can refresh the applications (and postings) lists live, plus a
 * best-effort title/company/status so a live toast can name the job without a
 * refetch race. The descriptive fields are OPTIONAL — an older emitter (or a
 * non-import change) may send only `applicationId`.
 */
export interface ApplicationChangedEvent {
  applicationId: string;
  /** Parsed job title, for a live notification ("Imported '<title>'"). */
  title?: string;
  /** Parsed company name, shown alongside the title. */
  company?: string;
  /** Resulting status id (e.g. `saved`, `applied`). */
  status?: string;
}

/**
 * Application-tracking capability (ADR 0001). The Generate trigger lives in the
 * `aiGenerations.save` flow (it upserts the Application as a side-effect); the two
 * creation triggers here are the doc-less ones: `track` (manual, → `applied`) and
 * `saveFromPosting` (Jobs-page Save, → `saved`).
 *
 * ## One contact per application
 *
 * `contactName`/`contactEmail` are **canonical** — the single primary contact
 * (recruiter / hiring manager / apply-by-email recipient). `recipientName`/
 * `recipientEmail` are **deprecated aliases** of them:
 *
 * | Direction | Behaviour |
 * | --------- | --------- |
 * | `update({ contactName })`   | writes the canonical field |
 * | `update({ recipientName })` | writes the SAME canonical field |
 * | `update({ contactName, recipientName })` | canonical wins; the alias is ignored |
 * | response (`list`/`get`)     | `recipientName === contactName` always (same for email) |
 *
 * Both email names go through the same server-side address validation, and an
 * invalid one rejects the whole patch with `{ error }`. New UI should read and
 * write `contactName`/`contactEmail` only.
 *
 * ## Follow-up reminders
 *
 * `nextActionAt` (epoch ms, nullable) is the reminder. A backend sweep raises a
 * notification (`kind: 'application.follow_up'`, route `/applications` with
 * `search.highlight = <id>`) once per due date for non-terminal applications;
 * moving or clearing `nextActionAt` re-arms it. `nextActionNotifiedAt` is the
 * read-only dedupe marker behind that "once" — it exists on the wire only so a
 * backup round trip does not re-announce delivered reminders, and `update()`
 * deliberately has no field for it. There is **no** counts command:
 * overdue/upcoming badges are derived client-side from the `nextActionAt` values
 * already carried by `list()` (see `features/applications/lib/pipeline.ts`).
 *
 * ## Email-derived status adjudication (v2)
 *
 * `get()`'s `events` now also carry {@link StatusEvent.source}/
 * {@link StatusEvent.confirmed}. A `source: 'email'` row with `confirmed: false`
 * is a provisional, auto-written transition (see the `email.match` notification)
 * that the timeline must render distinctly, with Accept/Reject affordances:
 * - `acceptStatusEvent` sets `confirmed` to `true` in place — the status itself is
 *   untouched (the auto-write already applied it).
 * - `rejectStatusEvent` reverts the status BY COMPARE-AND-SET (a status the user
 *   changed by hand in the meantime is never clobbered — the row is simply
 *   marked reviewed instead) and APPENDS a reversal event
 *   (`source: 'email_reject'`); `status_events` stays append-only, so the
 *   original row is never edited or deleted.
 *
 * **Both require {@link StatusEvent.eventId} — the id of the SPECIFIC row
 * being actioned, not just the application id.** Two provisional rows can
 * coexist (a confirmation email, then a later rejection email, both still
 * unreviewed); always pass the `eventId` of the exact row the Accept/Reject
 * affordance was rendered on. Both are idempotent no-ops (`{ success: true
 * }`, nothing changed) when `eventId` does not resolve to a pending
 * unconfirmed row for `id` — never an error a UI needs to branch on.
 * The invariant runs the other way, and it is the one that matters:
 * **`confirmed: false` marks exactly the rows a human has not ruled on
 * yet** — see {@link StatusEvent.confirmed}, which owns the rule and
 * enumerates what may write it. That is the entire safety model for a
 * classifier with a recorded precision limit (see `docs/knowledge/
 * decision-records/0013-email-confirmation-watching.md`).
 */
export interface ApplicationsContract {
  list(): Promise<Application[]>;
  get(id: string): Promise<ApplicationDetail>;
  /** Transition the status, optionally recording a free-text `note` — persisted
   *  on the appended `status_events` row and returned as `StatusEvent.note` by
   *  `get()` (the interaction log). */
  setStatus(args: {
    id: string;
    status: string;
    note?: string;
  }): Promise<ApplicationMutationResult>;
  /** Accept the SPECIFIC email-derived, unconfirmed status-event row
   *  `eventId` names — sets its {@link StatusEvent.confirmed} flag to `true`; the
   *  status itself is untouched. `eventId` must be the
   *  {@link StatusEvent.eventId} of the exact row the Accept affordance was
   *  rendered on — see {@link StatusEvent.eventId}'s doc for why "the most
   *  recent pending row" is not a safe substitute. A no-op when `eventId`
   *  does not resolve to a pending row for `id` (still `{ success: true }`,
   *  not an error). */
  acceptStatusEvent(args: { id: string; eventId: number }): Promise<ApplicationMutationResult>;
  /** Reject the SPECIFIC email-derived, unconfirmed status-event row
   *  `eventId` names — reverts the status by compare-and-set (never clobbers
   *  a status that moved on, whether by the user's own hand or a later
   *  email, in the meantime) and appends a reversal event. Same
   *  `eventId`-targeting requirement as {@link acceptStatusEvent}. A no-op
   *  when `eventId` does not resolve to a pending row. */
  rejectStatusEvent(args: { id: string; eventId: number }): Promise<ApplicationMutationResult>;
  update(req: ApplicationUpdateRequest): Promise<ApplicationMutationResult>;
  remove(args: { id: string; keepDocuments: boolean }): Promise<ApplicationMutationResult>;
  track(req: ApplicationTrackRequest): Promise<ApplicationCreateResult>;
  saveFromPosting(req: ApplicationTrackRequest): Promise<ApplicationCreateResult>;
  /** Subscribe to out-of-band application changes (e.g. browser-extension imports).
   *  Returns a sync unsubscribe handle. */
  onChanged(handler: (event: ApplicationChangedEvent) => void): () => void;
}

export const APPLICATIONS_CHANNELS = {
  list: 'applications:list',
  get: 'applications:get',
  setStatus: 'applications:setStatus',
  acceptStatusEvent: 'applications:acceptStatusEvent',
  rejectStatusEvent: 'applications:rejectStatusEvent',
  update: 'applications:update',
  remove: 'applications:remove',
  track: 'applications:track',
  saveFromPosting: 'applications:saveFromPosting',
} as const;
