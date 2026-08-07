import type { MatchResumeRequest, ResumeTrimSuggestionsRequest } from '../../schemas/index.js';
import type { MatchScore, TrimSuggestions } from '../../types/index.js';

export interface MatchContract {
  resume(req: MatchResumeRequest): Promise<MatchScore>;
  trimSuggestions(req: ResumeTrimSuggestionsRequest): Promise<TrimSuggestions>;
}

export const MATCH_CHANNELS = {
  resume: 'match:resume',
  trimSuggestions: 'match:trimSuggestions',
} as const;
