import { detectLanguage, toLanguageCode } from '@ajh/shared';

/** {@link resolveTargetLanguage}'s result: the language to actually use for
 *  THIS run, plus whether that answer is confident. `language` is always a
 *  real 2-letter code (every downstream consumer — the prompt, the date
 *  formatter, `resolveMarket` — needs one); `confident` is the separate axis
 *  that decides what may be REMEMBERED. See the function doc comment. */
interface TargetLanguageResolution {
  language: string;
  confident: boolean;
}

/**
 * Resolve the tailor run's target language — an explicit, ordered precedence
 * chain (owner decision, see the plan's "What to build" §1/§3): a GUESS must
 * never be preferred over a confident answer, and must never be REMEMBERED
 * as one either.
 *
 * 1. The persisted `targetLanguage` — the field the STAGED PIPELINE actually
 *    writes (`target_language` → `AiGenerationRecord.targetLanguage`,
 *    `commands/resume_pipeline/mod.rs:712`). #1003's "keep the regenerate
 *    language" branch read `resumeLanguage`/`jobAdLanguage` instead, which
 *    the staged pipeline leaves EMPTY (`empty_record()`), so it was dead on
 *    this flow — this is the fix.
 * 2. The persisted `jobAdLanguage` — the fast (AIGeneratePage) path's own
 *    field, a legitimate target per `metadata.ts:211` (`targetLanguage:
 *    jobAdLanguage`, i.e. the SAME "target = the ad's language" answer as
 *    tier 1, just from the other write path).
 * 3. A fresh, confident detection of the CURRENT job ad (`detectLanguage`
 *    already returns `'unknown'` rather than a low-confidence guess — <20
 *    chars, franc `'und'`, or an unmapped code).
 * 4. `'en'` — the LAST resort. Not a confident fact: every downstream
 *    consumer needs a concrete 2-letter code to run generation with, so this
 *    function cannot return "unknown" here. This is the ONLY tier where
 *    `confident` is `false`.
 *
 * `resumeLanguage` — the SOURCE résumé's language — is deliberately never
 * read. It is the second door the English-lock bug (Defect B) walks through:
 * an English résumé applying to a German job is not, on its own, evidence
 * the candidate wants an English document; the job ad's language is the only
 * legitimate signal for a TARGET.
 *
 * Each candidate is normalized ({@link toLanguageCode}) and validated
 * INDEPENDENTLY before the next tier is tried — the SAME persisted field
 * carries two shapes across writers (`extractMetadata`'s heuristic fallback
 * writes a display NAME like "German"; every other writer stores an ISO
 * code), and a short-circuit on presence-without-validity would send a
 * perfectly good lower tier to detection unread.
 *
 * `confident` is what CALLERS use to decide what may reach the wire: sending
 * the tier-4 guess to `session.start` and having Rust persist it verbatim
 * would let a FUTURE run's tier 1 prefer that guess forever — exactly the
 * bug this chain exists to close. See `start`'s own comment for how the
 * caller keeps a guess off the wire without a schema change (Rust's own
 * `ai_generations::merge_application`'s `pick` already treats an empty incoming field as "keep
 * whatever is stored").
 *
 * Known limit (not fixable here, not worth a test): the renderer detects
 * with **franc**, Rust's `validate::content` checks with **whatlang** — two
 * different third-party models can legitimately disagree on the same text.
 */
export function resolveTargetLanguage(
  // Deliberately its own small shape, not `Pick<AiGenerationRecord, …>`: both
  // fields are OPTIONAL here (a caller may not have a record at all yet),
  // where `AiGenerationRecord`'s own fields are always-present strings. A
  // real `AiGenerationRecord` (including one with `resumeLanguage` set) still
  // satisfies this structurally — see the pure-function tests, which pass a
  // full record to prove `resumeLanguage` is present on the input yet never
  // read.
  latestGeneration: { targetLanguage?: string; jobAdLanguage?: string } | undefined,
  jobDesc: string
): TargetLanguageResolution {
  const persistedTarget = toLanguageCode(latestGeneration?.targetLanguage ?? '');
  if (/^[a-z]{2}$/.test(persistedTarget)) {
    return { language: persistedTarget, confident: true };
  }
  const persistedJobAd = toLanguageCode(latestGeneration?.jobAdLanguage ?? '');
  if (/^[a-z]{2}$/.test(persistedJobAd)) {
    return { language: persistedJobAd, confident: true };
  }
  const detected = detectLanguage(jobDesc);
  if (detected !== 'unknown') {
    return { language: detected, confident: true };
  }
  return { language: 'en', confident: false };
}
