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
  update: 'applications:update',
  remove: 'applications:remove',
  track: 'applications:track',
  saveFromPosting: 'applications:saveFromPosting',
} as const;
