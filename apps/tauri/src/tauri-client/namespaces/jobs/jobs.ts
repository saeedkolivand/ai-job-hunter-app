import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { JobEvent } from '@ajh/shared/types';

import { asyncUnsub } from '../../utils.js';

export const jobs = {
  list: () => invoke('jobs_list'),
  get: (jobId: string) => invoke('jobs_get', { jobId }),
  cancel: (jobId: string) => invoke('jobs_cancel', { jobId }),
  retry: (jobId: string) => invoke('jobs_retry', { jobId }),
  onEvent: (handler: (event: JobEvent) => void) =>
    asyncUnsub(() => listen<JobEvent>('jobs:event', (e) => handler(e.payload))),
};
