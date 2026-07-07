/**
 * Anti-AI-tell writing rules. The centralized "sounds human, not machine-written"
 * ruleset shared across every generation surface (résumé, cover letter, referral,
 * application answers, inline rewrite).
 *
 * Distilled from the `humanizer` Claude skill and Wikipedia's "Signs of AI writing"
 * (the catalogue of giveaways: AI-vocabulary, promotional self-adjectives, vague
 * attributions, filler, em-dash overuse, rule-of-three, negative parallelisms,
 * "-ing" depth-faking, passive voice, abstraction over concrete fact).
 *
 * Two-tier split so each surface gets exactly the rules that are safe for it:
 * - {@link ANTI_AI_TELL_LEXICAL}. The shared lexical core: word/phrase bans only.
 *   SAFE inside a résumé bullet (no prose-flow, em-dash, or first-person rules that
 *   would fight ATS bullet conventions). This governs words the model INTRODUCES on
 *   its own; an exact job-ad keyword grounded in the résumé still wins.
 * - {@link ANTI_AI_TELL_PROSE}. {@link ANTI_AI_TELL_LEXICAL} PLUS prose-only flow
 *   rules (em-dash hard ban, sentence-rhythm variety, no rule-of-three, no negative
 *   parallelism, no "-ing" depth-faking, no needless passive, concrete over
 *   abstract). For letters, messages, and free-text answers.
 *
 * The bans above are necessary but not sufficient: removing AI-vocabulary doesn't
 * by itself make writing read as human-authored. {@link HUMANIZE_LEXICAL} and
 * {@link HUMANIZE_PROSE} are the POSITIVE counterpart, composed ALONGSIDE the bans
 * (never replacing them) at every call site:
 * - {@link HUMANIZE_LEXICAL}. Résumé/ATS-safe tier: specificity + bullet-shape
 *   variety only. No contractions, fragments, or other prose-imperfection — a
 *   résumé keeps its bullet/ATS register regardless of the requested tone.
 * - {@link HUMANIZE_PROSE}. Prose tier: sentence-cadence variance (burstiness),
 *   concrete-over-generic specifics, cut structural clichés, and tone-gated
 *   controlled imperfection (contractions/fragments allowed for a warm/casual
 *   register, never for a formal one, and never a typo or grammar error).
 *
 * Every technique here is bound by the honesty/anti-fabrication spine defined at
 * each call site (the grounding present/absent split, `ATS_PRECEDENCE`, the
 * per-surface HONESTY blocks): it changes HOW real content is written, never WHAT
 * is claimed.
 *
 * Self-consistency: these constants contain NO em or en dashes used as punctuation.
 * Normal hyphens appear only where a hyphen is genuinely part of a word.
 */
export const ANTI_AI_TELL_LEXICAL = `NATURAL VOICE (anti-AI-tell). Applies to any words YOU introduce, never to exact job-ad keywords already grounded in the résumé:
- Drop AI-vocabulary: delve, leverage, robust, seamless, cutting-edge, tapestry, testament, landscape (abstract), navigate (abstract), realm, underscore, showcase, foster, intricate, pivotal, vibrant, garner, vital, crucial, harness, elevate, streamline, unlock, empower. Use the plain word for the real thing instead.
- No promotional / inflated self-adjectives: passionate, results-driven, proven track record, team player, go-getter, synergy, dynamic, detail-oriented, world-class, cutting-edge.
- No vague attributions / weasel words: "studies show", "experts say", "industry reports", "it is widely known".
- Cut filler phrases: "in order to" -> "to", "due to the fact that" -> "because", "at this point in time" -> "now", "has the ability to" -> "can".`;

export const ANTI_AI_TELL_PROSE = `${ANTI_AI_TELL_LEXICAL}
PROSE FLOW (anti-AI-tell, for connected writing):
- EM-DASH HARD BAN: never use a long dash (em or en) anywhere. Replace it with a period, comma, colon, or parentheses.
- Vary sentence length and rhythm; do not run several same-shaped sentences in a row.
- No rule-of-three: do not force ideas into groups of three.
- No negative parallelisms ("not just X, but Y" / "it's not about X, it's about Y").
- No superficial "-ing" openers or tails (highlighting, showcasing, reflecting, ensuring, underscoring) that fake depth.
- No passive voice where active is natural.
- Concrete over abstract: name the real thing and what changed, not adjectives.`;

/**
 * Positive humanization, résumé/ATS-safe tier. Composed ALONGSIDE
 * {@link ANTI_AI_TELL_LEXICAL} (never replaces it): the lexical bans stop the
 * model reaching for detector red flags, this adds the affirmative moves that
 * make bullets read like one specific person's real work instead of ten
 * AI-templated clones. Deliberately NO prose-imperfection here (no
 * contractions, fragments, or loosened punctuation) — a résumé keeps ATS
 * parsing and a professional register regardless of the requested tone.
 */
export const HUMANIZE_LEXICAL = `SPECIFICITY AND BULLET VARIETY (adds to the bans above, never replaces them; resume tier stays ATS safe, no contractions or fragments):
- SPECIFICITY: prefer the real number, tool, or project name the resume already gives over a generic claim. "Cut checkout latency from 480ms to 90ms with Redis caching" beats "improved performance using caching".
- BULLET VARIETY: every bullet still opens with a strong past-tense action verb, but vary the verb and the sentence construction after it within a role so bullets are not identical Verb, What, Tech, metric templates. Rows of identically shaped bullets are the single biggest resume AI tell.
- This adds variety and specificity only. Every number, tool, and project still has to already be in the resume, exactly as the rules above require.`;

/**
 * Positive humanization, prose tier. Composed ALONGSIDE {@link ANTI_AI_TELL_PROSE}
 * (never replaces it): the bans remove AI tells, this adds the affirmative
 * techniques that make connected writing read like a person drafted it, not a
 * model optimizing for "sounds impressive". Every technique here is bound by
 * the honesty rules at each call site: it changes HOW real content is
 * written, never WHAT is claimed.
 */
export const HUMANIZE_PROSE = `SOUNDING HUMAN (positive moves; adds to the bans above, never replaces them):
- CADENCE: mix short, punchy sentences of about 3 to 7 words with longer ones of about 20 to 30 words. Never let three sentences in a row run the same length or shape; flat, uniform rhythm is the single biggest 2026 AI-detector tell.
- CONCRETE OVER GENERIC: reach for the exact number, tool, or project name the fenced resume already gives, and build around one real challenge and its outcome, instead of a claim so broad no one could disprove it.
- CUT THE CLICHES: no "not just X but Y", no hedging preambles ("it is important to note", "generally speaking"), no over-used transitions ("with that in mind", "building on this").
- CONTROLLED IMPERFECTION, gated to the requested register: a formal or precise voice stays clean, with no fragments or asides; a warmer or casual voice may use a contraction, one deliberate fragment, an asymmetric paragraph, or a natural aside. Either way, never a typo or a grammar mistake.
- VOICE: specific to THIS candidate and THIS company. Two or three things said well beat ten things name-dropped.
- None of this licenses inventing a detail: every number, tool, project, and challenge still has to come from the fenced resume, exactly as the honesty rules above require.`;

// ─── Output tone ──────────────────────────────────────────────────────────────

/**
 * The 4 output-tone settings the user can pick in Settings -> Output Tone.
 * Mirrors `OutputToneSchema` in
 * `apps/desktop/.../preferences-schema.ts` (kept in sync by hand, same pattern
 * as other cross-package literal unions such as `GenerationMode` — `prompts`
 * stays free of any app/renderer import).
 */
export type OutputTone = 'professional' | 'casual' | 'formal' | 'creative';

const TONE_DIRECTIVES: Record<OutputTone, string> = {
  professional:
    'TONE: polished, warm, and professional. The default voice for a job application: confident and human, not stiff.',
  casual:
    "TONE: conversational and casual. Contractions and more of the candidate's own voice are natural here, while staying a real job application, never sloppy.",
  formal:
    'TONE: formal and precise. Restrained, minimal imperfection, no contractions or fragments, but still vary sentence rhythm and lead with concrete specifics rather than reading stiff.',
  creative:
    "TONE: a more narrative, distinctive voice is welcome, told through the candidate's real story. Bounded: never gimmicky, cutesy, or unprofessional, this is still a job application.",
};

/**
 * Résumé/ATS-safe overrides for `toneDirective(tone, { lexical: true })`.
 * `casual`/`creative` otherwise license contractions or a narrative,
 * fragment-friendly voice, which contradicts the résumé's hard ATS-safe ban on
 * contractions/fragments (`HUMANIZE_LEXICAL`) — a contradiction a compliant
 * model resolves via `TONE_PRECEDENCE`, but a weak local model may not. These
 * variants adjust word choice, emphasis, and confidence only, never grammar or
 * structure. `professional`/`formal` are already ATS-safe as written, so they
 * need no override.
 */
const LEXICAL_TONE_OVERRIDES: Partial<Record<OutputTone, string>> = {
  casual:
    "TONE: conversational and casual, kept inside the resume's professional, ATS-safe register. Let word choice, emphasis, and confidence carry the candidate's voice, never a looser grammar or structure.",
  creative:
    "TONE: a more distinctive voice in word choice and emphasis is welcome, still inside the resume's professional, ATS-safe register. Bounded: never gimmicky or unprofessional, and never a looser grammar or structure, this is still a job application.",
};

/**
 * One-line voice directive for the selected output tone, composed ALONGSIDE the
 * honesty and market-convention rules at each call site (never overriding
 * them): tone shapes register and word choice; it never licenses inventing a
 * fact or dropping a structural requirement (salutation, sign-off, ATS bullet
 * format). Defaults to `professional` when omitted or unrecognized.
 * `{ lexical: true }` swaps in the résumé/ATS-safe variant (see
 * {@link LEXICAL_TONE_OVERRIDES}) for surfaces that must never reach for
 * contractions or prose imperfection; prose surfaces (cover letter,
 * application answers, referral, email) omit it and get the full directive.
 */
export function toneDirective(tone?: OutputTone, options?: { lexical?: boolean }): string {
  const key: OutputTone = tone && TONE_DIRECTIVES[tone] ? tone : 'professional';
  if (options?.lexical) return LEXICAL_TONE_OVERRIDES[key] ?? TONE_DIRECTIVES[key];
  return TONE_DIRECTIVES[key];
}
