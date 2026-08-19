import type {
  MatchResumeRequest,
  MatchTextRequest,
  ResumeTrimSuggestionsRequest,
} from '../../schemas/index.js';
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
  /**
   * Score one résumé against arbitrary job-ad TEXT — for a caller with a
   * `jobDesc: string` in hand but no `PostingsCache` id (e.g. the Score tab in
   * `JobAdView`, whose `TailorFlow` parent receives an `Application` /
   * `AutopilotFoundJob`, neither of which carries one). Routes through the
   * SAME shared kernel `resume()` does — not a second scorer — with two
   * deliberate, fixed differences: it is content-addressed on the job text
   * itself (repeated opens of the same posting are free), and semantic
   * (embedding) scoring is always OFF here, never caller-configurable —
   * mirrors `resume()`'s "omitting means keyword-only" default, made
   * unconditional for this ad-hoc surface. `scoreSource` is therefore always
   * `'keyword'` on the result.
   */
  text(req: MatchTextRequest): Promise<MatchScore>;
  trimSuggestions(req: ResumeTrimSuggestionsRequest): Promise<TrimSuggestions>;
}

export const MATCH_CHANNELS = {
  resume: 'match:resume',
  text: 'match:text',
  trimSuggestions: 'match:trimSuggestions',
} as const;
