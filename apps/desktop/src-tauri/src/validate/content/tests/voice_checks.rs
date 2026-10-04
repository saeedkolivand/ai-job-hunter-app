//! `voice`: sentence rhythm and the generic-letter check, and `voice.ai_tell_lexical` /
//! `voice.template_opener` for English and German — the checked tier fires, the prompt-only
//! tier never reaches a user.

use super::{support::*, *};

/// A letter that names nothing about the posting, in the language of the fixtures.
const GENERIC_LETTER: &str =
    "Dear Hiring Manager, I would be a great addition to your organisation and \
                              look forward to hearing from you soon. Best regards, Jane";

/// Burstiness needs [`voice::MIN_SENTENCES_FOR_BURSTINESS`] sentences before it
/// says anything — below that, uniformity is a coincidence.
#[test]
fn low_burstiness_needs_enough_sentences_and_real_uniformity() {
    let uniform = |count: usize| {
        (0..count)
            .map(|_| "The team shipped the release to production without any real trouble.")
            .collect::<Vec<_>>()
            .join(" ")
    };
    let short = uniform(voice::MIN_SENTENCES_FOR_BURSTINESS - 1);
    silent(&en_letter(&short), VOICE_LOW_BURSTINESS);

    let long = uniform(voice::MIN_SENTENCES_FOR_BURSTINESS);
    fired(&en_letter(&long), VOICE_LOW_BURSTINESS);

    // Varied prose of the same length must stay silent.
    let varied = "I shipped it. The retry scheduler was rewritten in Rust over two long \
                  weeks after the settlement backlog grew past anything the old design could \
                  absorb. Latency dropped. Then we moved on to the ledger itself, which took \
                  most of the following quarter and taught me more about idempotency than any \
                  book had. It worked. Barely. The second attempt held up under the Black \
                  Friday peak and has not needed a rollback since, which is the only metric \
                  I trust. Good enough.";
    silent(&en_letter(varied), VOICE_LOW_BURSTINESS);
}

/// Two triplets per ten sentences is the ceiling, not the trigger.
#[test]
fn rule_of_three_density_boundary() {
    let triplet = "We shipped the ledger, the scheduler, and the cache.";
    let plain = "The rollout held.";
    // 2 triplets in 10 sentences == the ceiling → silent.
    let at_limit = format!("{triplet} {triplet} {}", plain.repeat(8));
    silent(&en_letter(&at_limit), VOICE_RULE_OF_THREE_DENSITY);
    // 3 in 10 → over.
    let over = format!("{triplet} {triplet} {triplet} {}", plain.repeat(7));
    fired(&en_letter(&over), VOICE_RULE_OF_THREE_DENSITY);
}

/// One clause dash per [`voice::EM_DASH_WORDS_PER_ALLOWED`] words is natural;
/// a numeric range never counts.
#[test]
fn em_dash_overuse_ignores_numeric_ranges() {
    let ranges = "I worked at Acme from 2018–2021 and at Globex from 2021–2024 on the ledger.";
    silent(&en_letter(ranges), VOICE_EM_DASH_OVERUSE);
    let overused = "The ledger — which we rewrote — is fast now — faster than before.";
    let report = en_letter(overused);
    let hits = fired(&report, VOICE_EM_DASH_OVERUSE);
    assert_eq!(hits[0].severity, Severity::Warning);
}

#[test]
fn generic_letter_warns_when_nothing_is_posting_specific() {
    let report = en_letter(GENERIC_LETTER);
    let hits = fired(&report, VOICE_GENERIC_LETTER);
    assert_eq!(hits[0].severity, Severity::Warning);
    // The grounded fixture names the stack and must not fire.
    silent(&en_letter(EN_LETTER_GROUNDED), VOICE_GENERIC_LETTER);
}

/// F2 — `voice.generic_letter` measures the letter against the POSTING, so it
/// is a posting comparison and owes the module's own invariant: once the output
/// is not in the target language, every posting comparison is suppressed
/// (`alignment::validate` returns early on exactly this). Across two languages
/// the letter shares no vocabulary with the ad, so the check reported "0 things
/// specific to the posting" underneath the one finding that matters.
#[test]
fn generic_letter_is_suppressed_on_a_language_mismatch() {
    let german_letter = "Sehr geehrte Damen und Herren, hiermit bewerbe ich mich auf die \
                         ausgeschriebene Stelle in Ihrem Haus. Über eine Rückmeldung von Ihnen \
                         würde ich mich sehr freuen und stehe für ein Gespräch gerne zur \
                         Verfügung. Mit freundlichen Grüßen, Jane Doe";
    let report = en_letter(german_letter);
    fired(&report, CONTENT_LANGUAGE_MISMATCH);
    silent(&report, VOICE_GENERIC_LETTER);

    // …and a same-language letter that really is generic still fires, so the
    // guard suppresses the cascade rather than the check.
    fired(&en_letter(GENERIC_LETTER), VOICE_GENERIC_LETTER);
}

/// R9-F5 — `generic_letter` obeyed the generated-vs-target language gate but
/// not the JOB-AD-vs-target one, which is the normal DACH case: an
/// English-language posting for a German-speaking role. The keyword
/// intersection then ran across two languages, came back at ~0, and told a
/// letter that names the posting's own subject matter that it "could have been
/// sent to anyone".
#[test]
fn a_posting_in_another_language_does_not_make_a_letter_generic() {
    let en_ad = "Operations Lead, Payment Disputes\n\n\
                 We are hiring an operations lead for our chargeback and dispute desk. You \
                 will own the refund workflow end to end, cut the average handling time per \
                 case, and report weekly to the finance team on recovered volume.\n";
    let de_ad = "Teamleitung Zahlungsreklamationen\n\n\
                 Wir suchen eine Teamleitung für unsere Rückbuchungsstelle. Sie verantworten \
                 den Erstattungsprozess von Anfang bis Ende, senken die Bearbeitungszeit je \
                 Fall und berichten wöchentlich an die Finanzabteilung über das \
                 zurückgeholte Volumen.\n";
    let letter = "Jana Mustermann\njana.mustermann@example.com\n\n\
                  Sehr geehrtes Team,\n\n\
                  Ihre Ausschreibung nennt die Rückbuchungsstelle und den Erstattungsprozess \
                  als Kern der Aufgabe. Genau dort habe ich die letzten vier Jahre \
                  gearbeitet. Ich habe die Bearbeitungszeit je Fall von elf auf vier Tage \
                  gesenkt und das wöchentliche Berichtswesen an die Finanzabteilung \
                  aufgebaut.\n\n\
                  Mit freundlichen Grüßen\nJana Mustermann\n";
    let de_letter_against = |job_ad: &str| letter_in("de", letter, DE_SOURCE, job_ad);

    // The control: against the SAME posting in the target language, this letter
    // is measurably specific. Only the ad's language differs between the two.
    silent(&de_letter_against(de_ad), VOICE_GENERIC_LETTER);
    let across_languages = de_letter_against(en_ad);
    silent(&across_languages, VOICE_GENERIC_LETTER);

    // The premise is scoped to the ad↔document INTERSECTION and nothing else:
    // coverage compares the two DOCUMENTS against the same ad, symmetrically,
    // so it stays a real measurement and must survive.
    assert!(
        across_languages.metrics.keyword_coverage.is_some(),
        "a coverage comparison between two documents is not a cross-language \
         intersection; only `generic_letter` is"
    );

    // …and the control on the control: divergence has to be RELIABLY detected.
    // A terse ad is a keyword soup `whatlang` reads as anything at all, so
    // suppressing on that guess would switch the check off for ordinary short
    // postings. Below the reliability bar the check runs exactly as it always
    // did — including, accepted and pinned, its cross-language noise.
    let terse = "Payment disputes chargeback refund workflow lead";
    fired(&de_letter_against(terse), VOICE_GENERIC_LETTER);
}

/// F3 — the prompt's own ban is scoped: "Applies to any words YOU introduce,
/// never to exact job-ad keywords already grounded in the résumé"
/// (`natural-voice.ts`, the single source `lexicon.rs` is generated from). A
/// phrase the candidate's own résumé already uses is not a word the model
/// introduced, so flagging it made the validator stricter than the prompt it
/// exists to check — and told a finance candidate their own "leverage
/// calculator" was an AI tell.
///
/// The exemption is per-PHRASE: a source containing "leverage" exempts
/// "leverage" and nothing else.
#[test]
fn ai_tell_phrases_already_in_the_source_resume_are_exempt() {
    let source = "EXPERIENCE\n\nQuant Developer | Acme Capital | 2021 - Present\n\
                  - Built the leverage calculator the trading desk prices margin with\n";
    let generated = "EXPERIENCE\n\nQuant Developer | Acme Capital | 2021 - Present\n\
                     - Rebuilt the leverage calculator behind a seamless margin workflow\n";
    let report = report_against(generated, source);
    let phrases = evidence_of(&report, VOICE_AI_TELL_LEXICAL);
    assert!(
        !phrases.contains(&"leverage"),
        "the source résumé already says \"leverage\" — the model did not introduce it; \
         got {phrases:?}"
    );
    assert!(
        phrases.contains(&"seamless"),
        "\"seamless\" appears nowhere in the source, so the ban still applies to it; \
         got {phrases:?}"
    );
}

/// R4-F5 — the generated DE lexicon stores UNINFLECTED stems ("nahtlos",
/// "robust", "maßgeschneidert") and `contains_phrase` requires a word boundary
/// at BOTH ends, so the forms German actually writes — "nahtlose Integration",
/// "robuste Systeme", "maßgeschneiderte Lösungen" — never matched. The whole
/// German half of the AI-tell check was dead on real output.
#[test]
fn german_ai_tells_fire_on_inflected_forms() {
    let letter = "Sehr geehrte Damen und Herren,\n\n\
                  Ihre Plattform braucht eine nahtlose Integration der Zahlungsdienste. \
                  Ich habe robuste Systeme für den Zahlungsverkehr gebaut und \
                  maßgeschneiderte Lösungen für zwei Werke betreut.\n\n\
                  Mit freundlichen Grüßen\nJana Mustermann\n";
    let report = letter_in("de", letter, DE_SOURCE, DE_JOB_AD);
    let evidence = fired_evidence(&report, VOICE_AI_TELL_LEXICAL);
    for stem in ["nahtlos", "robust", "maßgeschneidert"] {
        assert!(
            evidence.contains(&stem),
            "the inflected form of {stem:?} must fire; got {evidence:?}"
        );
    }
}

/// The other half of R4-F5: the inflection tolerance is DE-only. English
/// matching keeps the exact both-ends word boundary it has always had, so
/// "harnesses" still does not fire the banned "harness".
#[test]
fn english_ai_tell_matching_is_unchanged_by_the_german_inflection_rule() {
    let letter = "Dear Hiring Manager,\n\n\
                  Your team harnesses a lot of data. I spent eight years on payment \
                  systems and shipped a ledger that settles twelve thousand orders a \
                  day. I read the posting twice before writing this.\n\n\
                  Best regards,\nJane Doe\n";
    silent(&en_letter(letter), VOICE_AI_TELL_LEXICAL);
}

// The `no-ai-slop` pattern catalog was curated into a CHECKED tier (fixed
// phrases carrying no factual content) and a PROMPT-ONLY tier (everything a
// real candidate might truthfully write, plus every construction rule) — see
// `AI_TELL_LEXICAL_WORDS_EN`'s doc in `natural-voice.ts` for the four-part
// test an entry has to clear. The split IS the decision, so both halves are
// pinned: the checked entries must fire, and the prompt-only vocabulary must
// never reach a user's document.

/// Every phrase the no-ai-slop pass added to the CHECKED tier fires.
///
/// Deliberately one fixture carrying all of them: an entry that silently
/// stopped matching (the shape of the German inflection bug above) would still
/// pass a test that only asserted "some AI tell fired".
#[test]
fn no_ai_slop_checked_tier_fires_on_a_slop_letter() {
    let letter = "Dear Hiring Manager,\n\n\
                  Your platform is widely regarded as the standard in payments. My \
                  meticulous approach to release engineering carried a multifaceted \
                  team through an ever-evolving market, and that was a genuine \
                  paradigm shift. It is worth noting that in today's world the work \
                  continues.\n\n\
                  Best regards,\nJane Doe\n";
    let report = en_letter(letter);
    let evidence = fired_evidence(&report, VOICE_AI_TELL_LEXICAL);
    for entry in [
        "widely regarded as",
        "meticulous",
        "multifaceted",
        "ever-evolving",
        "paradigm shift",
        "it is worth noting",
        "in today's world",
    ] {
        assert!(
            evidence.contains(&entry),
            "{entry:?} is on the prompt's own ban list and must fire; got {evidence:?}"
        );
    }
    assert!(
        report.ok,
        "voice findings stay advice — a model may never produce a Critical"
    );
}

/// The contraction spellings a model actually writes, in BOTH apostrophe
/// shapes.
///
/// [`flattened_lower`] normalizes case and whitespace but NOT punctuation, and
/// a model emits the typographic apostrophe (U+2019) about as often as the
/// ASCII one. That is why the first pass refused apostrophe entries outright:
/// either spelling would have been half-dead. `ai_tell_issues` now folds
/// U+2019 onto U+0027 before matching, so ONE ASCII entry covers both — this
/// test is what makes that fold load-bearing (drop it and the typographic half
/// goes silent).
#[test]
fn contraction_ai_tells_fire_with_either_apostrophe() {
    let typographic = "Dear Hiring Manager,\n\n\
                       It\u{2019}s worth noting that I built the settlement ledger you \
                       advertise for. It\u{2019}s important to note that it still runs \
                       every night. In today\u{2019}s world that is rarer than it \
                       sounds.\n\n\
                       Best regards,\nJane Doe\n";
    for (shape, letter) in apostrophe_shapes(typographic) {
        let report = en_letter(&letter);
        let evidence = fired_evidence(&report, VOICE_AI_TELL_LEXICAL);
        for entry in [
            "it's worth noting",
            "it's important to note",
            "in today's world",
        ] {
            assert!(
                evidence.contains(&entry),
                "{entry:?} must fire on the {shape} spelling; got {evidence:?}"
            );
        }
    }
}

/// The opener check owes the apostrophe fold too.
///
/// [`super::lexicon::TEMPLATE_OPENERS_EN`]'s shape rule is the same
/// one-directional rule the AI-tell arrays carry (U+0027 allowed, U+2019
/// banned), and the prompt-side catalog-shape test asserts it over ALL SIX
/// arrays — but `template_opener_issues` matched UNFOLDED text, so an
/// apostrophe-bearing opener entry was half-dead while the shape rule said it
/// was whole. Folding at this call site too (never inside the shared
/// `flattened_lower`, same reasoning as `ai_tell_issues`) makes the rule true
/// everywhere.
///
/// Its control lives in
/// [`german_template_openers_are_unaffected_by_the_apostrophe_fold`], a
/// SEPARATE test on purpose: inside one test the German half would sit behind
/// the English panic and prove nothing about the mutation.
#[test]
fn template_openers_fire_with_either_apostrophe() {
    let typographic = "Dear Hiring Manager,\n\n\
                       I\u{2019}m excited to apply for the payments role. At Acme I \
                       built the settlement ledger that clears twelve thousand orders \
                       a night, and I read your posting twice before writing.\n\n\
                       Best regards,\nJane Doe\n";
    for (shape, letter) in apostrophe_shapes(typographic) {
        let report = en_letter(&letter);
        assert_eq!(
            first_evidence(&report, VOICE_TEMPLATE_OPENER),
            Some("i'm excited to apply"),
            "{VOICE_TEMPLATE_OPENER} must report the contraction opener on the {shape} \
             spelling; the report carried {:?}",
            codes(&report)
        );
    }
}

/// German opener entries carry no apostrophe, so the fold above must be a
/// no-op for them. Separate from the English test so that removing the fold
/// leaves this one GREEN — which is what makes it a control rather than a
/// second copy of the same assertion.
#[test]
fn german_template_openers_are_unaffected_by_the_apostrophe_fold() {
    let german = "Sehr geehrte Damen und Herren,\n\n\
                  hiermit bewerbe ich mich auf die ausgeschriebene Stelle als \
                  Backend-Entwicklerin in Ihrem Unternehmen und freue mich sehr über \
                  eine Rückmeldung von Ihnen.\n\n\
                  Mit freundlichen Grüßen\nJana Mustermann\n";
    let report = letter_in("de", german, DE_SOURCE, DE_JOB_AD);
    assert_eq!(
        first_evidence(&report, VOICE_TEMPLATE_OPENER),
        Some("hiermit bewerbe ich mich"),
        "{VOICE_TEMPLATE_OPENER} must report the German opener on the apostrophe-free German \
         fixture; the report carried {:?}",
        codes(&report)
    );
}

/// The negative control the tiering exists for: a TRUTHFUL letter written
/// entirely out of the catalog's prompt-only vocabulary must stay silent.
///
/// Every phrase here either names something a real candidate really did
/// ("utilize", "facilitate", "embark", "supercharge"), carries a real domain
/// meaning ("beacon", as in a BLE/iBeacon fleet; "transformative justice", a
/// named social-work practice; "Paramount Global", a real employer), or is
/// ordinary human filler ("at the end of the day", "in conclusion", "as you can
/// see"). The prompt tells the model to avoid all of them; a Warning reading
/// "this is an AI tell" on one is a false accusation against the user, which
/// this module prices higher than a missed tell.
///
/// "Paramount" is the third-pass addition and the sharpest case of the class:
/// the per-phrase exemption in [`super::voice`] reads the SOURCE RÉSUMÉ only,
/// never the job ad, so a letter addressed to Paramount Global had the
/// employer's own name reported back as machine-written.
///
/// ## Why ONE `silent` covers BOTH lexicon arrays
///
/// There is no separate `voice.ai_tell_prose` CODE. On the letter path
/// [`super::voice::validate_letter`] calls `ai_tell_issues(ctx, true)`, which
/// merges [`super::lexicon::ai_tell_lexical`] and
/// [`super::lexicon::ai_tell_prose`] into one hit list and reports every hit
/// under [`VOICE_AI_TELL_LEXICAL`] — pinned from the positive side by
/// [`no_ai_slop_checked_tier_fires_on_a_slop_letter`], where the PROSE entries
/// "it is worth noting" and "in today's world" fire under exactly that code. So
/// promoting any phrase in this fixture into EITHER array fails the line below;
/// splitting the prose tier into its own code later would fail the positive
/// test first, which is where that decision has to be made.
///
/// [`VOICE_TEMPLATE_OPENER`] is a genuinely separate code and is pinned
/// separately. It is NOT pinned on the résumé control below: `voice::validate`
/// never calls `template_opener_issues` for a résumé, so the assertion could
/// not fail there for any edit — a guard that cannot fail reads as coverage and
/// is not.
#[test]
fn no_ai_slop_prompt_only_vocabulary_never_reaches_the_validator() {
    let letter = "Dear Hiring Manager,\n\n\
                  I chose to utilize Terraform to facilitate the AWS move at Acme, \
                  which for a payments team is table stakes. At its core the settlement \
                  system is a queue. When it comes to on-call, going forward I want \
                  fewer pages, not more. As you can see in terms of scale, the BLE \
                  beacon fleet I ran was the real game changer, and at the end of the \
                  day I would embark on this again. It did supercharge our deploys. \
                  Before that I coordinated the county's transformative justice pilot \
                  and taught a transformative learning module on Mezirow. The billing \
                  work I did for Paramount Global is the closest match to this role.\n\n\
                  In conclusion, with regard to the timeline, I can start in March.\n\n\
                  Best regards,\nJane Doe\n";
    let report = en_letter(letter);
    silent(&report, VOICE_AI_TELL_LEXICAL);
    silent(&report, VOICE_TEMPLATE_OPENER);
}

/// The same control on the RÉSUMÉ surface, where the lexical tier also runs.
/// Present-tense bullets are an ordinary convention for a current role, and
/// "Utilize" / "Facilitate" / "Spearhead" are exactly what a real résumé writes
/// — the class of word this pass deliberately kept out of the checked tier.
#[test]
fn no_ai_slop_prompt_only_verbs_are_silent_on_an_ordinary_resume() {
    let source = "EXPERIENCE\n\nPlatform Engineer | Acme Payments | 2021 - Present\n\
                  - Provision the AWS fleet with Terraform\n\
                  - Run the weekly release meeting\n\
                  - Own the warehouse beacon rollout\n";
    let generated = "EXPERIENCE\n\nPlatform Engineer | Acme Payments | 2021 - Present\n\
                     - Utilize Terraform to provision the AWS fleet across three regions\n\
                     - Facilitate the weekly release meeting for twelve engineers\n\
                     - Spearhead the BLE beacon rollout across forty warehouses\n";
    silent(&report_against(generated, source), VOICE_AI_TELL_LEXICAL);
}
