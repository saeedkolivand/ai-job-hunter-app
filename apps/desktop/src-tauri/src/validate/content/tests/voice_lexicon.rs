//! The generated lexicon's public interface and the Italian curation pass.

use super::{support::*, *};

// `lexicon.rs` is @generated and carries no hand-authored tests of its own;
// these exercise only its public interface (`lexicon::ai_tell_lexical` /
// `ai_tell_prose` / `template_openers`), never its private constants, so they
// stay valid across regeneration.

/// An UNCURATED language gets an empty list, never the English one.
///
/// The generated module's own contract (see its header): `natural-voice.ts`
/// hands a language with no curated list a generic, WORDLESS directive
/// (`genericAntiAiTellLexical`/`genericAntiAiTellProse`), so checking French
/// output against English words would flag phrasing the prompt never asked it
/// to avoid — accusing without evidence, the same rule
/// `documents::evidence::has_curated_function_words` enforces for the ATS
/// density ceiling. This test asserted the OLD English-fallback behaviour and
/// was left stale by the regeneration that changed it.
#[test]
fn lexicon_uncurated_language_gets_no_list_rather_than_english() {
    let en_lexical = lexicon::ai_tell_lexical("en");
    assert!(!en_lexical.is_empty(), "English is curated");
    assert!(!lexicon::ai_tell_lexical("de").is_empty(), "German is too");
    assert!(!lexicon::ai_tell_lexical("it").is_empty(), "Italian is too");
    for lang in ["fr", "es", "nl", "pt", "zz", ""] {
        assert!(
            lexicon::ai_tell_lexical(lang).is_empty(),
            "{lang} has no curated lexicon — it must not borrow English's"
        );
        assert!(lexicon::ai_tell_prose(lang).is_empty());
        assert!(lexicon::template_openers(lang).is_empty());
    }
    // German and Italian are genuinely different lists, not fallback aliases.
    assert_ne!(lexicon::ai_tell_lexical("de"), en_lexical);
    assert_ne!(lexicon::ai_tell_lexical("it"), en_lexical);
    assert_ne!(
        lexicon::ai_tell_lexical("it"),
        lexicon::ai_tell_lexical("de")
    );
    assert_ne!(
        lexicon::template_openers("de"),
        lexicon::template_openers("en")
    );
    assert_ne!(
        lexicon::template_openers("it"),
        lexicon::template_openers("en")
    );
}

/// Every entry across every language/tier must be lowercase and non-empty:
/// matching lowercases the haystack, so an upper-case entry would be dead,
/// and an empty entry would match everything. The DE prose tier is exempt
/// from the non-empty rule: every German prose tell turned out to be
/// construction-dependent (see `AI_TELL_PROSE_WORDS_DE`'s doc — the prompt
/// bans them as sentence-opener constructions a substring cannot judge), so
/// an empty list is the honest state, not a codegen accident.
#[test]
fn lexicon_every_entry_is_lowercase_and_non_empty() {
    let curated: [&[&str]; 8] = [
        lexicon::ai_tell_lexical("en"),
        lexicon::ai_tell_lexical("de"),
        lexicon::ai_tell_lexical("it"),
        lexicon::ai_tell_prose("en"),
        // Unlike DE, the Italian prose tier is non-empty (see
        // AI_TELL_PROSE_WORDS_IT's doc), so it belongs in the mandatory list.
        lexicon::ai_tell_prose("it"),
        lexicon::template_openers("en"),
        lexicon::template_openers("de"),
        lexicon::template_openers("it"),
    ];
    let check = |entry: &&str| {
        assert!(!entry.trim().is_empty(), "empty entry in lexicon");
        assert_eq!(
            *entry,
            entry.to_lowercase(),
            "lexicon entries must be lowercase; got {entry:?}"
        );
    };
    for list in curated {
        assert!(!list.is_empty(), "no curated list may be empty");
        list.iter().for_each(check);
    }
    // The DE prose tier may be empty, but any entry it ever gains must obey
    // the same per-entry rules as the curated lists.
    lexicon::ai_tell_prose("de").iter().for_each(check);
}

// `target_language` is a user-configurable output-language setting
// (`OUTPUT_LANGUAGES` in the renderer), not a detected property of the text —
// so Italian output reaches `ctx.lang == "it"` and these validators exactly
// the same way German output does. Fixtures are inline literals rather than
// shared `IT_SOURCE`/`IT_JOB_AD` fixture files (mirroring
// `german_template_openers_are_unaffected_by_the_apostrophe_fold` above): a
// short, generic `source_resume` keeps `content.language_mismatch` and the
// per-phrase source exemption out of the way of what each test actually
// checks.

const IT_SOURCE_STUB: &str = "Maria Rossi\nSviluppatrice\nmaria@example.com\n";

/// Every phrase the Italian curation pass added to the CHECKED tier fires, in
/// one letter — the same "one fixture, every entry" discipline
/// `no_ai_slop_checked_tier_fires_on_a_slop_letter` uses for English, so an
/// entry that silently stopped matching cannot hide behind "some AI tell
/// fired". `spirito di squadra` and `all'avanguardia` are deliberately ABSENT
/// (see `AI_TELL_LEXICAL_WORDS_IT`'s "Rejected" doc): both were demoted to
/// prompt-only after review, and their negative control lives in
/// [`italian_rejected_candidates_never_reach_the_validator`] below.
#[test]
fn italian_ai_tells_fire_on_a_slop_letter() {
    let letter = "Egregio selezionatore,\n\n\
                  Nel panorama odierno, in un mondo sempre più digitale, la vostra azienda \
                  offre un ventaglio di soluzioni cloud. Sono orientato ai risultati; la mia \
                  collega è invece orientata ai risultati in modo analogo. Sono una persona \
                  meticolosa e ho lavorato con un team meticoloso su ogni rilascio. Porto con \
                  me una comprovata esperienza nel settore dei pagamenti, e ho eccellenti \
                  capacità comunicative con i team distribuiti. Al fine di completare il \
                  progetto in tempo, e in considerazione del fatto che le scadenze erano \
                  strette, ho riorganizzato il team. Al momento attuale la piattaforma serve \
                  molti clienti. Studi dimostrano che i team affiatati consegnano prima, e gli \
                  esperti concordano su questo punto. Come è noto a tutti, la qualità del \
                  codice conta più della velocità. È importante sottolineare che il sistema \
                  non si è mai fermato, e vale la pena notare che ho scritto ogni riga del \
                  backend.\n\n\
                  Cordiali saluti,\nMaria Rossi\n";
    let report = letter_in("it", letter, IT_SOURCE_STUB, "");
    let evidence = fired_evidence(&report, VOICE_AI_TELL_LEXICAL);
    for entry in [
        "un ventaglio di",
        "orientato ai risultati",
        "orientata ai risultati",
        "meticoloso",
        "meticolosa",
        "comprovata esperienza",
        "eccellenti capacità comunicative",
        "al fine di",
        "in considerazione del fatto che",
        "al momento attuale",
        "studi dimostrano",
        "gli esperti concordano",
        "come è noto a tutti",
        "nel panorama odierno",
        "in un mondo sempre più",
        "è importante sottolineare",
        "vale la pena notare",
    ] {
        assert!(
            evidence.contains(&entry),
            "{entry:?} is on the prompt's own Italian ban list and must fire; got {evidence:?}"
        );
    }
    assert!(
        report.ok,
        "voice findings stay advice — a model may never produce a Critical"
    );
}

/// The negative control for the two demotions
/// (`AI_TELL_LEXICAL_WORDS_IT`'s "Rejected" doc): a letter that uses BOTH
/// phrases in exactly the truthful, domain-legitimate context that motivated
/// demoting them must stay silent — the same trust bar
/// `no_ai_slop_prompt_only_vocabulary_never_reaches_the_validator` holds
/// English to. `spirito di squadra` sits in OBJECT position as a genuine
/// coaching achievement (rule 2); `all'avanguardia` names the real Italian
/// Futurist art movement (rule 4).
#[test]
fn italian_rejected_candidates_never_reach_the_validator() {
    let letter = "Gentile Selezionatore,\n\n\
                  Prima di entrare nel settore tech ho lavorato come allenatore: ho costruito \
                  lo spirito di squadra di ventidue atleti in tre stagioni, e nel tempo libero \
                  ho dedicato la tesi di laurea all'avanguardia futurista italiana. Oggi \
                  gestisco il sistema di liquidazione che elabora le transazioni ogni \
                  notte.\n\n\
                  Cordiali saluti,\nMaria Rossi\n";
    let report = letter_in("it", letter, IT_SOURCE_STUB, "");
    silent(&report, VOICE_AI_TELL_LEXICAL);
}

/// The Italian elision mutation: `in riferimento all'annuncio` puts an
/// apostrophe INSIDE a checked opener in ordinary, unremarkable Italian,
/// unlike English where it only shows up in a handful of contractions. A
/// model writes the typographic apostrophe (U+2019) about as often as the
/// ASCII one; without `fold_apostrophes` on `template_opener_issues`, the
/// typographic spelling goes silently unmatched — which is exactly what this
/// test would catch if the fold were removed.
///
/// `ai_tell_issues` shares the SAME `fold_apostrophes` function (see
/// `AI_TELL_PROSE_WORDS_IT`'s doc for why a second, lexical-tier Italian
/// elision fixture was not added here too): its fold is already proven by
/// `contraction_ai_tells_fire_with_either_apostrophe`'s EN contraction twins.
#[test]
fn italian_template_opener_fires_with_either_apostrophe() {
    let typographic = "Gentile Selezionatore,\n\n\
                       In riferimento all\u{2019}annuncio per la posizione, ho sviluppato il \
                       sistema di liquidazione che elabora le transazioni ogni notte per una \
                       piattaforma di pagamenti.\n\n\
                       Cordiali saluti,\nMaria Rossi\n";
    for (shape, letter) in apostrophe_shapes(typographic) {
        let report = letter_in("it", &letter, IT_SOURCE_STUB, "");
        assert_eq!(
            first_evidence(&report, VOICE_TEMPLATE_OPENER),
            Some("in riferimento all'annuncio"),
            "{VOICE_TEMPLATE_OPENER} must report the opener on the {shape} spelling; the \
             report carried {:?}",
            codes(&report)
        );
    }
}

/// The negative control: a truthful, professionally-written Italian letter
/// that never reaches for a curated tell must stay silent on both voice
/// codes — the same trust bar `no_ai_slop_prompt_only_vocabulary_never_reaches_the_validator`
/// holds English to.
#[test]
fn clean_professional_italian_letter_has_no_voice_ai_tell_or_opener_issues() {
    let letter = "Gentile Dott.ssa Bianchi,\n\n\
                  Ho seguito con attenzione la crescita della vostra piattaforma di pagamenti \
                  negli ultimi due anni, in particolare il passaggio a un'architettura basata \
                  su microservizi. Nel mio ruolo attuale gestisco il sistema di liquidazione \
                  che elabora le transazioni notturne per Acme Pagamenti, e credo che la mia \
                  esperienza con Kubernetes e PostgreSQL si adatti bene al vostro stack.\n\n\
                  Cordiali saluti,\nMaria Rossi\n";
    let report = letter_in("it", letter, IT_SOURCE_STUB, "");
    silent(&report, VOICE_AI_TELL_LEXICAL);
    silent(&report, VOICE_TEMPLATE_OPENER);
}

/// The résumé/letter tier split holds for Italian too:
/// `voice::validate` (résumé path) runs the lexical tier only, so a
/// LEXICAL-tier entry fires on a bullet while a PROSE-tier (letter-register)
/// entry — one that cannot occur in an ATS bullet — never does.
#[test]
fn italian_lexical_tier_applies_to_resume_bullets_prose_tier_does_not() {
    let source = "ESPERIENZA\n\nIngegnere Piattaforma | Acme Pagamenti | 2021 - Presente\n\
                  - Gestisco il sistema di liquidazione\n";
    let generated = "ESPERIENZA\n\nIngegnere Piattaforma | Acme Pagamenti | 2021 - Presente\n\
                     - Un ventaglio di competenze in pagamenti digitali che copre ogni notte \
                     le transazioni, nel panorama odierno sempre più critico per il settore\n";
    let report = report_in("it", generated, source, "");
    let evidence = fired_evidence(&report, VOICE_AI_TELL_LEXICAL);
    assert!(
        evidence.contains(&"un ventaglio di"),
        "the lexical-tier entry must fire on a résumé bullet; got {evidence:?}"
    );
    assert!(
        !evidence.contains(&"nel panorama odierno"),
        "the prose-tier (letter-only) entry must NOT fire on a résumé bullet; got {evidence:?}"
    );
}
