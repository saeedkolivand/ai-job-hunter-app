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
