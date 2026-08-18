import type { MatchResumeRequest, ResumeTrimSuggestionsRequest } from '../../schemas/index.js';
import type { MatchScore, TrimSuggestions } from '../../types/index.js';

export interface MatchContract {
  /**
   * Score one résumé against one job. The single scoring path: the jobs list
   * asks for a score per row as that row renders, rather than running one pass
   * over everything (the one-shot `match_resume_batch` command was removed —
   * it had no consumers).
   *
   * Keyword-only by default. Semantic (embedding) scoring is opt-in per
   * request via `semanticScoringEnabled`; omitting it means keyword-only, not
   * "provider decides".
   */
  resume(req: MatchResumeRequest): Promise<MatchScore>;
  trimSuggestions(req: ResumeTrimSuggestionsRequest): Promise<TrimSuggestions>;
}

export const MATCH_CHANNELS = {
  resume: 'match:resume',
  trimSuggestions: 'match:trimSuggestions',
} as const;
