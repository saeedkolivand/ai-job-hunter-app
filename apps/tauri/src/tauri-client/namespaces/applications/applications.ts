import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type {
  ApplicationChangedEvent,
  ApplicationTrackRequest,
  ApplicationUpdateRequest,
} from '@ajh/shared';

import { asyncUnsub } from '../../utils.js';

export const applications = {
  list: () => invoke('applications_list'),
  get: (id: string) => invoke('applications_get', { id }),
  setStatus: ({ id, status, note }: { id: string; status: string; note?: string }) =>
    invoke('applications_set_status', { id, status, note }),
  update: (req: ApplicationUpdateRequest) => invoke('applications_update', { req }),
  // `keepDocuments` reaches the Rust command as the snake_case `keep_documents` arg.
  remove: ({ id, keepDocuments }: { id: string; keepDocuments: boolean }) =>
    invoke('applications_delete', { id, keepDocuments }),
  track: (req: ApplicationTrackRequest) => invoke('applications_track', { req }),
  saveFromPosting: (req: ApplicationTrackRequest) =>
    invoke('applications_save_from_posting', { req }),
  // The bridge (and any out-of-band creator) emits `applications:changed` with
  // `{ applicationId, title?, company?, status? }` — see
  // `extension_bridge::APPLICATIONS_CHANGED_EVENT`.
  onChanged: (handler: (event: ApplicationChangedEvent) => void) =>
    asyncUnsub(() =>
      listen<ApplicationChangedEvent>('applications:changed', (e) => handler(e.payload))
    ),
};
