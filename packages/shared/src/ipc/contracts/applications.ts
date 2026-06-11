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
