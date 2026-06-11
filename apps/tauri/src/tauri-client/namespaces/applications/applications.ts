import { invoke } from '@tauri-apps/api/core';

import type { ApplicationTrackRequest, ApplicationUpdateRequest } from '@ajh/shared';

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
};
