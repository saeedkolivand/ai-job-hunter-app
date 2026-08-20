import { invoke } from '@tauri-apps/api/core';

import type {
  MatchResumeRequest,
  MatchTextRequest,
  ResumeTrimSuggestionsRequest,
} from '@ajh/shared/schemas';

export const match = {
  resume: (req: MatchResumeRequest) => invoke('match_resume', { req }),
  text: (req: MatchTextRequest) => invoke('match_resume_text', { req }),
  trimSuggestions: (req: ResumeTrimSuggestionsRequest) =>
    invoke('resume_trim_suggestions', { req }),
};
