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
 */
export interface ApplicationsContract {
  list(): Promise<Application[]>;
  get(id: string): Promise<ApplicationDetail>;
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
