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
   * SAME shared kernel `resume()` does, over the SAME pre-processed text (the
   * Rust command strips markdown before scoring, exactly as `resume()` does
   * for a cached posting) — not a second scorer — but two axes still
   * legitimately diverge from `resume()`: this call never has a title or
   * requirements to compose in (only the description `JobAdView` holds), and
   * semantic (embedding) scoring is always OFF here, never
   * caller-configurable, so its number only matches `resume()`'s when
   * semantic scoring is off there too. Content-addressed on the pre-processed
   * job text, so repeated opens of the same posting reuse that cached score.
   * `scoreSource` is therefore always `'keyword'` on the result.
   */
  text(req: MatchTextRequest): Promise<MatchScore>;
  trimSuggestions(req: ResumeTrimSuggestionsRequest): Promise<TrimSuggestions>;
}

export const MATCH_CHANNELS = {
  resume: 'match:resume',
  text: 'match:text',
  trimSuggestions: 'match:trimSuggestions',
} as const;
