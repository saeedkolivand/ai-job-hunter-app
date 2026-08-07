import { invoke } from '@tauri-apps/api/core';

import type { MatchResumeRequest, ResumeTrimSuggestionsRequest } from '@ajh/shared/schemas';

export const match = {
  resume: (req: MatchResumeRequest) => invoke('match_resume', { req }),
  trimSuggestions: (req: ResumeTrimSuggestionsRequest) =>
    invoke('resume_trim_suggestions', { req }),
};
