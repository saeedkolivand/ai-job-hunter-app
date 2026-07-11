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
 * - {@link antiAiTellLexical}. The shared lexical core: word/phrase bans only.
 *   SAFE inside a résumé bullet (no prose-flow, em-dash, or first-person rules that
 *   would fight ATS bullet conventions). This governs words the model INTRODUCES on
 *   its own; an exact job-ad keyword grounded in the résumé still wins.
 * - {@link antiAiTellProse}. {@link antiAiTellLexical} PLUS prose-only flow
 *   rules (em-dash hard ban, sentence-rhythm variety, no rule-of-three, no negative
 *   parallelism, no "-ing" depth-faking, no needless passive, concrete over
 *   abstract). For letters, messages, and free-text answers.
 *
 * LANGUAGE-AWARE: both functions take the target output language (ISO-639-1, e.g.
 * "en"/"de" — matches `GenerationMeta.targetLanguage`) and pick the ruleset for it:
 * - `en` — the original, curated English lists (unchanged).
 * - `de` — a curated German AI-tell (KI-Floskeln) lexicon + prose rules, not a
 *   translation of the English list (a literal translation would ban words no
 *   German writer would reach for and miss the actual German tells).
 * - anything else — a generic, language-referencing directive (avoid stock AI
 *   phrasing / literal translations of English AI clichés, vary sentence length,
 *   use natural idiom for that language). We only curate a word list for a
 *   language we can actually verify; for the rest, naming the target language and
 *   pointing at the failure mode is honest and still useful. A newly-added output
 *   language therefore needs zero change here — it automatically gets the generic
 *   directive.
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
const ANTI_AI_TELL_LEXICAL_EN = `NATURAL VOICE (anti-AI-tell). Applies to any words YOU introduce, never to exact job-ad keywords already grounded in the résumé:
- Drop AI-vocabulary: delve, leverage, robust, seamless, cutting-edge, tapestry, testament, landscape (abstract), navigate (abstract), realm, underscore, showcase, foster, intricate, pivotal, vibrant, garner, vital, crucial, harness, elevate, streamline, unlock, empower. Use the plain word for the real thing instead.
- No promotional / inflated self-adjectives: passionate, results-driven, proven track record, team player, go-getter, synergy, dynamic, detail-oriented, world-class, cutting-edge.
- No vague attributions / weasel words: "studies show", "experts say", "industry reports", "it is widely known".
- Cut filler phrases: "in order to" -> "to", "due to the fact that" -> "because", "at this point in time" -> "now", "has the ability to" -> "can".`;

const ANTI_AI_TELL_PROSE_EN = `${ANTI_AI_TELL_LEXICAL_EN}
PROSE FLOW (anti-AI-tell, for connected writing):
- EM-DASH HARD BAN: never use a long dash (em or en) anywhere. Replace it with a period, comma, colon, or parentheses.
- Vary sentence length and rhythm; do not run several same-shaped sentences in a row.
- No rule-of-three: do not force ideas into groups of three.
- No negative parallelisms ("not just X, but Y" / "it's not about X, it's about Y").
- No superficial "-ing" openers or tails (highlighting, showcasing, reflecting, ensuring, underscoring) that fake depth.
- No passive voice where active is natural.
- Concrete over abstract: name the real thing and what changed, not adjectives.`;

/**
 * Curated German (de) equivalent of {@link ANTI_AI_TELL_LEXICAL_EN} — the actual
 * KI-Floskeln (AI clichés) a German-language model reaches for, not a translation
 * of the English list (English AI tells like "delve" or "leverage" have no direct
 * German equivalent a model would actually produce).
 */
const ANTI_AI_TELL_LEXICAL_DE = `NATURAL VOICE (Anti-KI-Floskeln). Gilt für Wörter, die DU selbst einbringst, nie für exakte, im Lebenslauf belegte Schlüsselbegriffe aus der Stellenanzeige:
- Vermeide KI-Floskeln: "darüber hinaus", "nahtlos", "robust", "spielt eine entscheidende Rolle", "in der heutigen Zeit", "in der heutigen Welt", "in einem dynamischen Umfeld", "mit großer Begeisterung", "Leidenschaft für", "eine spannende Herausforderung", "maßgeschneidert", "innovativ" (als Füllwort), "eintauchen in". Nutze stattdessen das konkrete Wort für die Sache selbst.
- Kein werbliches Eigenlob: "ergebnisorientiert", "Teamplayer", "nachgewiesene Erfolgsbilanz", "dynamisch" (als Selbstbeschreibung), "detailorientiert", "Weltklasse".
- Keine vagen Verweise oder Weichmacher: "Studien zeigen", "Experten sagen", "es ist allgemein bekannt".
- "nicht nur ... sondern auch" (der deutsche Zwilling des englischen "not just X but Y" KI-Tells) nur sparsam einsetzen, nie als wiederkehrendes Stilmittel.
- Vermeide formelhafte Nominalstil-Einstiege (ein Satz, der um ein abstraktes Substantiv plus schwaches Verb gebaut ist, z. B. "Die Umsetzung von X erfolgte durch..."); greife stattdessen zu einem direkten, verbführenden Satz.`;

const ANTI_AI_TELL_PROSE_DE = `${ANTI_AI_TELL_LEXICAL_DE}
PROSE-FLUSS (Anti-KI-Floskeln, für zusammenhängenden Text):
- Langer Gedankenstrich (Halbgeviert- oder Gedankenstrich) verboten: nie einsetzen, stattdessen Punkt, Komma, Doppelpunkt oder Klammern nutzen.
- Variiere Satzlänge und -rhythmus; nutze natürliche Nebensatzkonstruktionen statt mehrerer gleichförmiger Hauptsätze hintereinander.
- Kein Dreiklang-Zwang: presse Ideen nicht in Dreiergruppen.
- Kein identischer Absatzanfang: jeder Absatz beginnt anders als der vorherige.
- Konkret statt abstrakt: benenne die reale Sache und was sich geändert hat, nicht Adjektive.`;

/**
 * English display names for the generic-directive locales, purely so the
 * directive reads naturally ("in French" not "in fr"). Falls back to the raw
 * ISO-639-1 code for anything not listed — a newly-added `OUTPUT_LANGUAGES` entry
 * still gets a correct, if slightly terser, directive with zero change here.
 */
const LANGUAGE_DISPLAY_NAMES: Record<string, string> = {
  fr: 'French',
  es: 'Spanish',
  it: 'Italian',
  tr: 'Turkish',
  pt: 'Portuguese',
  ru: 'Russian',
  zh: 'Chinese',
  ja: 'Japanese',
  ko: 'Korean',
  nl: 'Dutch',
};

function languageDisplayName(code: string): string {
  return LANGUAGE_DISPLAY_NAMES[code] ?? code;
}

/**
 * Generic, language-referencing lexical directive for any output language we
 * don't have a curated lexicon for. Deliberately does NOT invent a word list for
 * a language we can't verify (a guessed lexicon risks banning words a native
 * speaker would actually use) — instead it names the target language and points
 * the model at the real failure mode: literal-translating an English AI cliche.
 */
function genericAntiAiTellLexical(code: string): string {
  const name = languageDisplayName(code);
  return `NATURAL VOICE (anti-AI-tell) for ${name} output. Applies to any words YOU introduce, never to exact job-ad keywords already grounded in the résumé:
- Avoid stock AI phrasing and cliches in ${name}. Never produce a direct, literal translation of an English AI cliche (delve into, leverage, robust, seamless, unlock the potential, and similar) into ${name}; use the natural, idiomatic word or phrase a native ${name} speaker would actually reach for.
- No promotional self-adjectives or vague weasel-word attributions ("studies show", "experts say") in ${name} either, including their local equivalents.
- Cut filler phrases and stock transitions; say the plain thing directly.`;
}

function genericAntiAiTellProse(code: string): string {
  const name = languageDisplayName(code);
  return `${genericAntiAiTellLexical(code)}
PROSE FLOW (anti-AI-tell, for connected ${name} writing):
- Long dash (em or en dash) hard ban: never use one anywhere. Replace it with a period, comma, colon, or parentheses.
- Vary sentence length and structure; do not run several same-shaped sentences in a row.
- No rule-of-three: do not force ideas into groups of three.
- Use natural idiom and sentence rhythm for ${name}, not a pattern copied from English AI writing.
- Concrete over abstract: name the real thing and what changed, not adjectives.`;
}

/** Normalize any incoming language value to a 2-letter lowercase code (default "en"). */
function normalizeLanguageCode(language?: string): string {
  return (language ?? 'en').trim().slice(0, 2).toLowerCase() || 'en';
}

/**
 * Lexical-tier anti-AI-tell ruleset for `language` (ISO-639-1, default "en").
 * Word/phrase bans only — safe inside a résumé bullet. See the module doc
 * comment above for the per-language design (curated en/de, generic elsewhere).
 */
export function antiAiTellLexical(language?: string): string {
  const code = normalizeLanguageCode(language);
  if (code === 'de') return ANTI_AI_TELL_LEXICAL_DE;
  if (code === 'en') return ANTI_AI_TELL_LEXICAL_EN;
  return genericAntiAiTellLexical(code);
}

/**
 * Prose-tier anti-AI-tell ruleset for `language` (ISO-639-1, default "en"):
 * {@link antiAiTellLexical} PLUS prose-flow rules. For letters, messages, and
 * free-text answers. See the module doc comment above for the per-language design.
 */
export function antiAiTellProse(language?: string): string {
  const code = normalizeLanguageCode(language);
  if (code === 'de') return ANTI_AI_TELL_PROSE_DE;
  if (code === 'en') return ANTI_AI_TELL_PROSE_EN;
  return genericAntiAiTellProse(code);
}

/**
 * Positive humanization, résumé/ATS-safe tier. Composed ALONGSIDE
 * {@link antiAiTellLexical} (never replaces it): the lexical bans stop the
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
 * Positive humanization, prose tier. Composed ALONGSIDE {@link antiAiTellProse}
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
