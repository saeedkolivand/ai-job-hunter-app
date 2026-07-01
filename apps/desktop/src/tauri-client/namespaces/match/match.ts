import { invoke } from '@tauri-apps/api/core';

import type { MatchResumeBatchRequest, MatchResumeRequest } from '@ajh/shared/schemas';

export const match = {
  resume: (req: MatchResumeRequest) => invoke('match_resume', { req }),
  resumeBatch: (req: MatchResumeBatchRequest) => invoke('match_resume_batch', { req }),
};
