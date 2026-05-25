import type { JobEvent, JobRecord } from '../../types/index.js';

export interface JobsContract {
  list(): Promise<JobRecord[]>;

  get(jobId: string): Promise<JobRecord | null>;

  cancel(jobId: string): Promise<void>;

  retry(jobId: string): Promise<void>;

  onEvent(handler: (event: JobEvent) => void): () => void;
}

export const JOBS_CHANNELS = {
  list: 'jobs:list',
  get: 'jobs:get',
  cancel: 'jobs:cancel',
  retry: 'jobs:retry',
  event: 'jobs:event',
} as const;
