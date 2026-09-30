use super::super::prompts::{
    draft_language_retry_note, draft_system, humanize_system, language_name, letter_system,
    repair_system, section_order_prompt_list, HumanizeTier,
};

/// The draft prompt must order a TRANSLATION, not just describe the target
/// language — this is the generation-side half of the cross-language-résumé
/// fix. The rule has to coexist with the grounding/fidelity clauses, not
/// replace them: an English source résumé translated into German still has
/// to draw every fact from `<candidate_resume>`, and employment entries still
/// carry their company/title/dates verbatim.
///
/// Mutation check: delete the translate bullet (leaving the rest of `Structure:`
/// untouched) — RAN, went red (no `TRANSLATE` in the output), reverted.
#[test]
fn the_draft_prompt_orders_a_translation() {
    let prompt = draft_system("de", "de");
    assert!(prompt.contains("TRANSLATE"));
    assert!(prompt.contains("German"));
    assert!(prompt.contains(
        "Every factual claim about the candidate MUST be traceable to a line in <candidate_resume>"
    ));
    assert!(prompt.contains("exactly as given"));
}

/// Every SYSTEM prompt in this file must name the target language in English
/// ("German") rather than interpolate the bare 2-char code ("de") a model
/// reads as an abbreviation, not an instruction.
///
/// Mutation check: revert `letter_system`'s `system_language_name` call back
/// to `system_language` — RAN, went red on EVERY language in the loop (not
/// just `de`), because the bare-code assertion is now parameterized on the
/// loop variable instead of hard-coded to `", in de."`; reverted.
#[test]
fn every_system_prompt_names_the_language_rather_than_its_code() {
    for (lang, name) in [
        ("de", "German"),
        ("fr", "French"),
        ("ja", "Japanese"),
        ("en", "English"),
    ] {
        let draft = draft_system(lang, "intl");
        let repair = repair_system(lang, false);
        let letter = letter_system(lang, "intl", false, false);
        let humanize = humanize_system(HumanizeTier::Resume, lang);
        let bare_code = format!(", in {lang}.");
        for prompt in [&draft, &repair, &letter, &humanize] {
            assert!(
                prompt.contains(name),
                "{lang}: expected {name:?} in {prompt:?}"
            );
            assert!(
                !prompt.contains(&bare_code),
                "{lang}: bare code leaked through"
            );
        }
    }

    // The `(in {lang})` clause inside the subject-line rule reads the SAME
    // name, not a second unconverted interpolation.
    let de_letter = letter_system("de", "de", false, false);
    assert!(de_letter.contains("subject line labelled \"Betreff\" (in German), on its own line"));
}

/// A language `language_name` has no curated entry for falls back to the bare
/// code — the ADR-010-safe fallback `language_name`'s doc comment promises.
/// `draft_system` must still build a usable prompt for it.
///
/// Mutation check: change the fallback arm from `other => other` to a fixed
/// string — RAN, went red (`language_name("xx")` stopped equaling `"xx"`),
/// reverted.
#[test]
fn an_uncurated_language_falls_back_to_the_bare_code() {
    assert_eq!(language_name("xx"), "xx");
    let prompt = draft_system("xx", "intl");
    assert!(prompt.contains("in xx"));
}

/// After a partial repair, the untouched sibling sections may still be in the
/// SOURCE language — the repair prompt must pin the output language over
/// whatever the siblings demonstrate, or the model imitates their language
/// right back.
///
/// Mutation check: revert to the bare "Match the language, voice and tense you
/// OBSERVE there" wording (drop the override sentence) — RAN, went red (no
/// "output language is German" clause), reverted.
#[test]
fn the_repair_prompt_pins_the_output_language_over_sibling_context() {
    let prompt = repair_system("de", true);
    assert!(prompt.contains("The output language is German, whatever the siblings are written in"));
}

/// The draft retry's corrective note names the language and never leaks the
/// bare code — it is read by a model, not logged for a human who already
/// knows the ISO tag.
///
/// Mutation check: interpolate the raw `lang` argument instead of
/// `system_language_name(lang)` — RAN, went red (`German` disappeared, the
/// literal `de-DE` code appeared instead), reverted.
#[test]
fn draft_language_retry_note_names_the_language() {
    let note = draft_language_retry_note("de-DE");
    assert!(note.contains("German"));
    assert!(!note.contains(" in de"));
}

/// `section_order_prompt_list` renders every section as `lang`'s localized
/// header, not the raw English `SectionId` debug word — the producer half of
/// the reported bug (a German résumé rendered "Projekte" and "Ausbildung &
/// Sprachen" as body text: the model invented German names for headings the
/// prompt handed it in English).
///
/// Mutation check: revert the function body to `format!("{id:?}")` per id
/// (dropping the `resume_conventions`/`.header` lookup) — RAN, went red on
/// both the "de" and "it" assertions (`Projects`/`Certifications` and
/// `Progetti`/`Certificazioni` respectively — the German and Italian
/// assertions fail in OPPOSITE directions, `contains` vs `!contains`, so a
/// revert cannot pass by accident), reverted.
#[test]
fn section_order_prompt_list_localizes_every_section_not_just_the_first_four() {
    let en = section_order_prompt_list("en", "us");
    assert!(en.contains("Professional Summary, Work Experience, Skills, Projects, Education"));

    let de = section_order_prompt_list("de", "de");
    assert!(de.contains("Projekte"), "Projects must be localized");
    assert!(
        de.contains("Zertifikate"),
        "Certifications must be localized"
    );
    assert!(!de.contains("Projects"));
    assert!(!de.contains("Certifications"));

    let it = section_order_prompt_list("it", "it");
    assert!(it.contains("Progetti"), "Projects must be localized");
    assert!(
        it.contains("Certificazioni"),
        "Certifications must be localized"
    );
    assert!(!it.contains("Projects"));
    assert!(!it.contains("Certifications"));
}

/// Producer vocabulary vs recogniser buckets, swept exhaustively: no heading
/// `resume_conventions` can emit, in any curated locale, may classify as a
/// DIFFERENT section's kind. Landing on `SectionKind::Other` is fine (the
/// recogniser simply has no bucket for Certifications/Languages/Awards);
/// landing on the wrong one is not — a regenerate would then rewrite the
/// wrong section.
///
/// `locale::resume`'s own collision test states the rejected German
/// alternatives and the reasoning behind them, but it names its cases as
/// literals and covers only de + it. This is the mechanical half: it names no
/// heading at all, so a locale or id added later is swept without anyone
/// remembering to extend a list. It found the Portuguese `Competências` →
/// `Other` gap fixed in `SKILLS_HEADINGS`.
///
/// Mutation check: removed `"competênc"` from `SKILLS_HEADINGS` — RAN, went
/// red on pt/Skills, restored.
#[test]
fn no_localized_heading_lands_in_another_sections_bucket() {
    use crate::documents::evidence::{classify_section, SectionKind};
    use crate::pipeline::resume::prompt_blocks::{resume_conventions, RESUME_CONVENTION_LOCALES};

    // Only the ids the recogniser actually has a bucket for; the rest may
    // legitimately answer `Other` and are asserted not to steal a bucket.
    let expected = |id: &str| match id {
        "Summary" => Some(SectionKind::Summary),
        "Experience" => Some(SectionKind::Experience),
        "Education" => Some(SectionKind::Education),
        "Skills" => Some(SectionKind::Skills),
        "Projects" => Some(SectionKind::Projects),
        _ => None,
    };

    for &lang in RESUME_CONVENTION_LOCALES {
        let conventions = resume_conventions(lang);
        for id in conventions.ids() {
            let heading = conventions.header(id);
            let kind = classify_section(heading);
            match expected(id) {
                // Exactly `want`, not `want || Other`. Allowing `Other` was
                // the first draft and it made the sweep unable to fail: a
                // heading the recogniser simply does not know reads the same
                // as one it knows correctly. Measured across all 7 locales,
                // every bucketed id already lands on its own bucket, so the
                // slack bought nothing and cost the whole guard.
                Some(want) => assert_eq!(
                    kind, want,
                    "locale {lang:?} heading {heading:?} (SectionId::{id}) classifies as \
                     {kind:?}, not {want:?} — a regenerate would rewrite the wrong section"
                ),
                None => assert_eq!(
                    kind,
                    SectionKind::Other,
                    "locale {lang:?} heading {heading:?} (SectionId::{id}) has no bucket of \
                     its own, so anything but Other means it was filed under another section"
                ),
            }
        }
    }
}

/// `ResumeConventions::header` falls back to the raw `SectionId` debug word on
/// a miss instead of panicking, and nothing in the type system ties the TS
/// `ResumeSectionHeaderId` union to the `SectionId`s `section_order_for`
/// actually emits. So the fallback is reachable by ordinary maintenance —
/// adding `SectionId::Volunteer` to an order const compiles, typechecks, and
/// passes `gen:prompts:check`, and the only symptom is the literal English
/// word `Volunteer` inside a prompt demanding Italian. (The ADR-010 face of
/// the same hole: a `SectionId::Custom(s)` in an order const would render
/// user-derived text straight into the SYSTEM slot.)
///
/// This is the guard for that. It never names an id: it enumerates whatever
/// `section_order_for` emits and asserts every curated locale has a real
/// entry for each.
///
/// Mutation check: added `SectionId::Volunteer` to `EUROPASS_ORDER` (then `IT_ORDER`) — RAN, went red
/// (`cargo test` and `pnpm gen:prompts:check` both stayed green, which is the
/// whole point), reverted.
#[test]
fn every_ordered_section_id_has_a_real_localized_header() {
    use crate::pipeline::resume::prompt_blocks::{resume_conventions, RESUME_CONVENTION_LOCALES};

    // Every market `locale::resume::section_order_for` branches on, plus an
    // unknown one for the default arm. A market added there without a line
    // here is still covered on the ID axis as long as it reuses an existing
    // order; a market with a BRAND NEW order const is the one case that needs
    // this list extended, which is why the default arm is asserted too.
    for market in ["us", "de", "at", "ch", "dach", "it", "zz"] {
        for id in crate::locale::resume::section_order_for(market) {
            let debug_name = format!("{id:?}");
            for &lang in RESUME_CONVENTION_LOCALES {
                // Membership, NOT `header(id) != id`. The value comparison
                // looks equivalent and is not: French for "Certifications" is
                // "Certifications", so a curated, correct entry would read as
                // a fallback. Asking whether the KEY exists is the property
                // that actually distinguishes a translation from a miss.
                assert!(
                    resume_conventions(lang)
                        .ids()
                        .any(|have| have == debug_name),
                    "market {market:?} orders SectionId::{debug_name}, but locale \
                     {lang:?} has no header entry for it — `header` would silently \
                     hand the model the English word {debug_name:?} while telling \
                     it to write {lang}"
                );
            }
        }
    }
}
