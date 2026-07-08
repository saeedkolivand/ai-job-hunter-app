import { invoke } from '@tauri-apps/api/core';

import type { MatchResumeRequest } from '@ajh/shared/schemas';

export const match = {
  resume: (req: MatchResumeRequest) => invoke('match_resume', { req }),
};
