// TODO(gen:prompts): replaced by generated file
//
//! Anti-AI-tell vocabulary, mirrored from the prompt side.
//!
//! **Hand-written stub.** The lists below are transcribed from
//! `packages/prompts/src/generate/natural-voice/natural-voice.ts` — the same
//! bans the generation prompt hands the model — so the validator checks exactly
//! what the prompt asked for. A codegen step
//! (`packages/prompts/scripts/gen-prompts-rust.ts`) replaces this file with a
//! generated one carrying the identical interface; keep the three function
//! signatures stable.
//!
//! `lang` is an ISO-639-1 code (`"en"`, `"de"`); anything else falls back to
//! English, matching `normalizeLanguageCode` on the TS side. Entries are
//! lowercase and matched with word boundaries (see
//! `super::contains_phrase`), so `vital` never fires on `revitalize`.

/// Word/phrase bans safe inside a résumé bullet — the lexical tier of
/// `antiAiTellLexical()`. No prose-flow rules here.
pub fn ai_tell_lexical(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => AI_TELL_LEXICAL_DE,
        _ => AI_TELL_LEXICAL_EN,
    }
}

/// Prose-only patterns from `antiAiTellProse()` + `HUMANIZE_PROSE`'s
/// "cut the clichés" list — negative parallelisms, "-ing" depth-faking, and
/// hedging preambles. Only meaningful for connected writing (cover letters,
/// summaries), never for a bullet.
pub fn ai_tell_prose(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => AI_TELL_PROSE_DE,
        _ => AI_TELL_PROSE_EN,
    }
}

/// Stock cover-letter openers. Not present in `natural-voice.ts` today — the
/// codegen task may add a `templateOpeners` export there, at which point this
/// list becomes generated like the other two.
pub fn template_openers(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => TEMPLATE_OPENERS_DE,
        _ => TEMPLATE_OPENERS_EN,
    }
}

/// Transcribed from `ANTI_AI_TELL_LEXICAL_EN`: the AI-vocabulary list, the
/// promotional self-adjectives, the vague attributions, and the filler phrases.
const AI_TELL_LEXICAL_EN: &[&str] = &[
    // AI vocabulary
    "delve",
    "leverage",
    "robust",
    "seamless",
    "cutting-edge",
    "tapestry",
    "testament",
    "realm",
    "underscore",
    "showcase",
    "foster",
    "intricate",
    "pivotal",
    "vibrant",
    "garner",
    "vital",
    "crucial",
    "harness",
    "elevate",
    "streamline",
    "unlock",
    "empower",
    // Promotional / inflated self-adjectives
    "passionate",
    "results-driven",
    "proven track record",
    "team player",
    "go-getter",
    "synergy",
    "detail-oriented",
    "world-class",
    // Vague attributions / weasel words
    "studies show",
    "experts say",
    "industry reports",
    "it is widely known",
    // Filler phrases
    "in order to",
    "due to the fact that",
    "at this point in time",
    "has the ability to",
];

/// Transcribed from `ANTI_AI_TELL_PROSE_EN` + `HUMANIZE_PROSE`. The prose
/// directive bans shapes rather than words, so these are the concrete patterns
/// it names: negative parallelisms, "-ing" depth-faking openers/tails, and the
/// hedging preambles / over-used transitions.
const AI_TELL_PROSE_EN: &[&str] = &[
    "not just",
    "not only",
    "it's not about",
    "it is not about",
    "highlighting",
    "showcasing",
    "underscoring",
    "it is important to note",
    "generally speaking",
    "with that in mind",
    "building on this",
];

/// Transcribed from `ANTI_AI_TELL_LEXICAL_DE` — the actual KI-Floskeln, not a
/// translation of the English list.
const AI_TELL_LEXICAL_DE: &[&str] = &[
    "darüber hinaus",
    "nahtlos",
    "robust",
    "spielt eine entscheidende rolle",
    "in der heutigen zeit",
    "in der heutigen welt",
    "in einem dynamischen umfeld",
    "mit großer begeisterung",
    "leidenschaft für",
    "eine spannende herausforderung",
    "maßgeschneidert",
    "eintauchen in",
    // Werbliches Eigenlob
    "ergebnisorientiert",
    "teamplayer",
    "nachgewiesene erfolgsbilanz",
    "detailorientiert",
    "weltklasse",
    // Vage Verweise
    "studien zeigen",
    "experten sagen",
    "es ist allgemein bekannt",
];

/// Transcribed from `ANTI_AI_TELL_PROSE_DE`: the German twin of the
/// "not just X but Y" tell plus the formulaic Nominalstil opener.
const AI_TELL_PROSE_DE: &[&str] = &["nicht nur", "sondern auch", "erfolgte durch"];

/// Stock English cover-letter openers — the phrases a letter that could have
/// been addressed to anyone starts with.
const TEMPLATE_OPENERS_EN: &[&str] = &[
    "i am writing to apply",
    "i am writing to express",
    "i am writing in response to",
    "i am excited to apply",
    "i would like to apply for the position",
    "please accept this letter",
    "with great interest i read",
    "i hope this message finds you well",
    "i hope this email finds you well",
    "as a passionate",
];

/// Stock German cover-letter openers (Standardfloskeln).
const TEMPLATE_OPENERS_DE: &[&str] = &[
    "hiermit bewerbe ich mich",
    "hiermit möchte ich mich bewerben",
    "mit großem interesse habe ich ihre stellenanzeige",
    "mit großem interesse habe ich gelesen",
    "auf ihre stellenanzeige hin",
    "wie ihrer stellenanzeige zu entnehmen ist",
];

#[cfg(test)]
mod test {
    use super::*;

    /// An unknown language falls back to English, exactly like
    /// `normalizeLanguageCode` on the prompt side — never to an empty list,
    /// which would silently disable the whole check.
    #[test]
    fn unknown_language_falls_back_to_english() {
        for lang in ["fr", "zz", "", "EN"] {
            assert_eq!(
                ai_tell_lexical(lang),
                AI_TELL_LEXICAL_EN,
                "{lang} must fall back to the English lexicon"
            );
            assert_eq!(ai_tell_prose(lang), AI_TELL_PROSE_EN);
            assert_eq!(template_openers(lang), TEMPLATE_OPENERS_EN);
        }
        assert_eq!(ai_tell_lexical("de"), AI_TELL_LEXICAL_DE);
        assert_eq!(template_openers("de"), TEMPLATE_OPENERS_DE);
    }

    /// Every entry must be lowercase and non-empty: matching lowercases the
    /// haystack, so an upper-case entry would be dead, and an empty entry would
    /// match everything.
    #[test]
    fn every_entry_is_lowercase_and_non_empty() {
        let all = [
            AI_TELL_LEXICAL_EN,
            AI_TELL_LEXICAL_DE,
            AI_TELL_PROSE_EN,
            AI_TELL_PROSE_DE,
            TEMPLATE_OPENERS_EN,
            TEMPLATE_OPENERS_DE,
        ];
        for list in all {
            assert!(!list.is_empty(), "no list may be empty");
            for entry in list {
                assert!(!entry.trim().is_empty(), "empty entry in lexicon");
                assert_eq!(
                    *entry,
                    entry.to_lowercase(),
                    "lexicon entries must be lowercase; got {entry:?}"
                );
            }
        }
    }
}
