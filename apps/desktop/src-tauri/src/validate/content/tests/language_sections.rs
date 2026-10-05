//! The per-section language check: a single drifted section, and the sections that
//! must never trip it.

use super::{support::*, *};

/// The blind spot the document-level majority vote leaves open: a single
/// section drifted to another language inside an otherwise-English résumé.
/// First proves the premise (the WHOLE-document read stays clean — the
/// English majority hides the one Italian section), then proves the
/// per-section pass catches exactly what the document-level one cannot.
#[test]
fn a_single_drifted_section_is_caught_even_though_the_document_reads_clean() {
    assert!(
        !is_language_mismatch(EN_EXPERIENCE_DRIFTED_ITALIAN, "en"),
        "premise: the document-level majority vote must NOT fire here — one \
         Italian section inside a mostly-English résumé is exactly the case \
         that hides from a whole-document read"
    );

    let report = en_resume(EN_EXPERIENCE_DRIFTED_ITALIAN, &en_requirements());
    let hits = fired(&report, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
    assert!(!report.ok);
    assert_eq!(
        hits[0].section.as_deref(),
        Some("EXPERIENCE"),
        "the finding must name the drifted section, not just the document"
    );
}

/// **Confirmation-review finding 4 (MEDIUM).** `char::is_lowercase()` is
/// `false` for every character in a caseless script (Arabic, Hebrew, CJK,
/// Thai, Devanagari), so a naive lowercase-word-RATIO reads 0 for a section
/// written in one and `looks_like_prose` skips it — the exact same drifted-
/// Experience-section shape [`a_single_drifted_section_is_caught_even_though_the_document_reads_clean`]
/// proves is caught in Italian must ALSO be caught in Arabic, and before this
/// commit it was (via the `SectionKind` allowlist this replaced, which never
/// asked whether the text "looked like prose" at all).
#[test]
fn a_drifted_experience_section_in_a_caseless_script_is_still_caught() {
    let generated = "Jane Doe\n\
        jane.doe@example.com | +49 30 1234567 | github.com/janedoe\n\n\
        SUMMARY\n\n\
        Eight years of backend work, most of it on payment systems and the \
        container platforms behind them.\n\n\
        EXPERIENCE\n\n\
        مهندس أول للأنظمة الخلفية في شركة أكمي للمدفوعات من عام 2021 حتى الآن\n\
        - قمت بخفض زمن الاستجابة عند الدفع من 480 مللي ثانية إلى 90 مللي ثانية \
        من خلال إضافة ذاكرة تخزين مؤقت أمام خدمة دفتر الحسابات\n\
        - قمت بتشغيل حاويات دوكر على عنقود كوبرنيتيس يستجيب لاثني عشر ألف طلب \
        في كل ثانية\n\n\
        PROJECTS\n\n\
        **Ledger CLI** · https://github.com/janedoe/ledger\n\
        Rust · SQLite\n\
        Double-entry bookkeeping for freelancers.\n\n\
        SKILLS\n\n\
        Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis\n\n\
        EDUCATION\n\n\
        BSc Computer Science, TU Berlin, 2014 - 2018\n";
    assert!(
        !is_language_mismatch(generated, "en"),
        "premise: the document-level majority vote must NOT fire here — one \
         Arabic section inside a mostly-English résumé is exactly the case \
         that hides from a whole-document read"
    );
    let report = en_resume(generated, &en_requirements());
    let hits = fired(&report, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
    assert!(!report.ok);
    assert_eq!(
        hits[0].section.as_deref(),
        Some("EXPERIENCE"),
        "the finding must name the drifted section, not just the document; got {hits:#?}"
    );
}

/// No-false-positive REGRESSION pin for a long, padded Title-Case skills list
/// — relabelled honestly, not a mutation-checked guard for any one gate.
/// **This fixture is suppressed THREE independent ways at once**, so no
/// single-gate mutation can turn it red here (verified, not assumed — see the
/// premise assertions below, each anchored to the actual SECTION BODY
/// [`section_language_issues`] validates, `"KENNTNISSE\n" + the padded list`,
/// not to the short unpadded `list` a prior version of this test asserted its
/// premise against while validating the padded one):
///
/// 1. `classify_section("KENNTNISSE")` is [`SectionKind::Skills`], so the
///    per-section pass's `SectionKind::Skills` filter excludes it outright.
/// 2. A Title-Case, middot-separated list opens every "word" uppercase, so
///    [`looks_like_prose`] reads it as a LIST, not prose, and excludes it too.
/// 3. `detected_language` maps this exact list to `None` — whatlang
///    confidently but WRONGLY reads Title-Case comma/middot tool lists as
///    Catalan, a language `documents::keywords::locale_tag_of` does not
///    curate (see
///    `documents::keywords::tests::language::detect_locale_tag_and_detected_language_agree_whenever_both_answer`)
///    — so the confidence-and-coverage gate excludes it a third time.
///
/// Each gate's OWN mutation-checked guard lives elsewhere, on a fixture that
/// isolates it: the `SectionKind::Skills` filter's guard is
/// [`a_lowercase_canonical_tool_list_in_skills_never_trips_the_per_section_language_check`]
/// (a LOWERCASE tool list, so it clears `looks_like_prose` and reads as a
/// COVERED-but-wrong language, leaving the Skills filter as the only thing
/// standing); `detected_language`'s confidence floor's guard is
/// [`a_correct_certifications_block_never_trips_the_per_section_language_check`].
/// This fixture keeps its place anyway as a straightforward "a realistic long
/// skills list never earns a false Critical" regression, now labelled for
/// what it actually is.
#[test]
fn a_long_title_case_skills_list_is_a_no_false_positive_regression() {
    let list = "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis";
    let padded_list =
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis · \
         Go · TypeScript · GraphQL · gRPC · Kafka · RabbitMQ · Elasticsearch · Prometheus · \
         Grafana · Jenkins · GitLab CI · Helm · Istio · Envoy · Vault · Consul · Nomad · \
         Ansible · Chef · Puppet · Nginx · HAProxy · Cassandra · MongoDB · ClickHouse";
    let padded = DE_CLEAN.replace(list, padded_list);
    // The exact text `section_language_issues` runs its checks over for the
    // KENNTNISSE section: `section_text` joins the heading and every line
    // with `\n` — this is what the premises below must be anchored to, not
    // the short `list` alone.
    let section_body = format!("KENNTNISSE\n{padded_list}\n");
    assert!(
        significant_chars(&section_body) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: the validated section body must clear the per-section char floor, or gate 3 \
         below proves nothing"
    );
    assert!(
        !looks_like_prose(&section_body),
        "premise: a Title-Case, middot-separated list must NOT read as prose — gate 2"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(&section_body),
        None,
        "premise: this ordinary Title-Case tool list is not a covered language under \
         detected_language — gate 3"
    );
    let report = report_in("de", &padded, DE_SOURCE, DE_JOB_AD);
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// **Confirmation-review finding 1 (HIGH).** `looks_like_prose` replaced a
/// `SectionKind` allowlist that was the ONLY thing keeping
/// `SectionKind::Skills` out of the per-section language check. Canonical
/// tool-name casing IS lowercase (`pandas`, `numpy`, `git`, `nginx`, `dbt`) —
/// the opposite of what the doc this replaces claimed ("has not been
/// observed") — and [`PROSE_LOWERCASE_WORD_RATIO`] (0.2) is one word in five,
/// so a genuinely lowercase tool list clears it easily. Measured on commit
/// 8c74ccd1's own branch: a truthful German `KENNTNISSE` section with a
/// Python/data stack earned a false Critical.
///
/// **Premise restated for `detected_language`, and deliberately a DIFFERENT
/// tool list than before.** The original fixture (a `pandas`/`numpy`/… list)
/// reads as Catalan under `whatlang` — like the Title-Case list above, that
/// is not a covered language, so `detected_language` already answers `None`
/// and the mutation check would be vacuous. This fixture is chosen because it
/// confidently reads as **French** — a language `documents::keywords::locale_tag_of`
/// DOES curate — so `detected_language` returns `Some("fr")` and the Skills
/// exclusion is doing real, provable work, not guarding against nothing.
///
/// Mutation check: drop the `section.kind != SectionKind::Skills` filter
/// from `section_language_issues` and this goes red.
#[test]
fn a_lowercase_canonical_tool_list_in_skills_never_trips_the_per_section_language_check() {
    let lowercase_tools = LOWERCASE_TOOL_LIST;
    let generated = DE_CLEAN.replace(SKILLS_LINE, lowercase_tools);
    assert!(
        significant_chars(lowercase_tools) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: the lowercase tool list alone must clear the per-section char floor"
    );
    assert!(
        looks_like_prose(lowercase_tools),
        "premise: a genuinely lowercase tool list reads as PROSE under the \
         0.2 ratio — this is the exact false positive the SectionKind::Skills \
         exclusion exists to catch, not a hypothetical one"
    );
    assert!(
        matches!(
            crate::documents::keywords::detected_language(lowercase_tools),
            Some(found) if found != "de"
        ),
        "premise: whatlang confidently (and wrongly) reads this lowercase tool list as some \
         OTHER covered language than the target \"de\" — the exact property this test needs \
         (which language whatlang names is a whatlang implementation detail, not this crate's \
         contract), so detected_language does not already suppress this on its own; the Skills \
         exclusion is what is doing the work"
    );
    let report = report_in("de", &generated, DE_SOURCE, DE_JOB_AD);
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// Regression: `SectionKind` has six variants and `classify_section` has no
/// heading list for Certifications, Awards, Publications or Languages-spoken —
/// they all land in `SectionKind::Other`. A Skills-only exclusion therefore
/// admitted exactly the shape it was written to keep out: a correct, English
/// certifications block is proper-noun-heavy, clears the char floor, and reads
/// as non-English. Firing there would blank `keywordCoverage` and suppress
/// every alignment finding on an otherwise-perfect résumé.
///
/// **Fixture changed, premise restated.** The original Title-Case bullet
/// fixture never actually exercised the language comparison at all: its
/// lowercase-initial-word ratio is 0.0, so `looks_like_prose` filters it out
/// before `detected_language` is ever consulted — a "passes for the wrong
/// reason" gap pre-dating this fix, surfaced while restating this premise
/// rather than carried forward silently. This fixture keeps the block's
/// proper-noun density (still confidently misdetected — French, confidence
/// ~0.13, well under `MIN_DETECTION_CONFIDENCE`) but lowercase-leads each
/// line so it genuinely clears the prose ratio, so the test exercises
/// `detected_language`'s CONFIDENCE gate specifically (this section is
/// `SectionKind::Other`, not `Skills`, so the Skills exclusion plays no part
/// here at all).
///
/// Mutation check: delete the confidence gate in
/// `documents::keywords::detected_language` (fall straight through to
/// `locale_tag_of`) and this goes red — see
/// `documents::keywords::tests::language::detected_language_goes_quiet_below_the_confidence_floor`,
/// which pins the same fixture shape's confidence directly.
#[test]
fn a_correct_certifications_block_never_trips_the_per_section_language_check() {
    let certs = "

CERTIFICATIONS
aws certified solutions architect - professional (2022)
                 google cloud professional data engineer (2023)
                 certified kubernetes administrator cka (2021)
";
    let body = certs.trim_start();
    assert!(
        significant_chars(body) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: this block must clear the per-section char floor, or the test proves \
         nothing about the exclusion"
    );
    assert!(
        looks_like_prose(body),
        "premise: this block must clear the prose ratio, or the test silently exercises \
         nothing but the looks_like_prose filter — the exact gap being fixed here"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(body),
        None,
        "premise: whatlang reads this correct ENGLISH certifications block as French with \
         LOW confidence — the confidence gate is doing the real work, not a coincidence"
    );
    let with_certs = format!("{EN_CLEAN}{certs}");
    let report = en_resume(&with_certs, &en_requirements());
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// **PR #1003 finding 1 (CRITICAL).** A Volunteer heading is real —
/// `export::parser`'s `SECTION_NAMES` promotes it to a `SectionHeader` line —
/// but `classify_section`'s six variants have no arm for it, so it lands in
/// `SectionKind::Other` exactly like Certifications/Awards do. Unlike those,
/// `pipeline::resume::stages::sections::key_of` maps `SectionKind::Other` to
/// no `SectionKey`, so `criticals_by_section` cannot route a Critical
/// labelled here to anything `repair` can regenerate — and the run's own
/// regenerate button has no section to target either. A Critical would park
/// the run at `needsReview` behind a finding nothing can ever clear, so this
/// must fire as a Warning, not a Critical.
///
/// Mutation check: drop the `section.kind == SectionKind::Other` guard from
/// `section_language_issues` (i.e. always leave the table's declared
/// Critical) and this goes red — the hit reads Critical.
///
/// **PR #1003 finding 1 (CRITICAL), Awards variant.** Same unroutable-section
/// shape as the Volunteer test above, proven on a second heading
/// `SECTION_NAMES` knows and `classify_section` does not, so the fix is
/// proven on more than the one heading that happened to motivate it.
///
/// Mutation check: same as the Volunteer test above.
#[test]
fn a_drifted_volunteer_section_warns_rather_than_blocks() {
    for (heading, article, name, section) in [
        (
            "VOLUNTEER",
            "a",
            "Volunteer",
            "\n\nVOLUNTEER\n\n\
        Ho aiutato una piccola organizzazione no profit locale a digitalizzare i \
        propri archivi cartacei, costruendo uno strumento di catalogazione che i \
        volontari potessero usare senza alcuna formazione tecnica.\n",
        ),
        (
            "AWARDS",
            "an",
            "Awards",
            "\n\nAWARDS\n\n\
        Ho ricevuto il premio Employee of the Year per aver guidato la migrazione \
        della piattaforma di pagamenti verso la nuova architettura a container, \
        riducendo i tempi di inattività del servizio durante il cambio.\n",
        ),
    ] {
        let generated = format!("{EN_CLEAN}{section}");
        assert!(
            !is_language_mismatch(&generated, "en"),
            "premise: the document-level majority vote must NOT fire — one drifted \
             {name} section inside an otherwise-English résumé is exactly the \
             case the section-level pass exists to catch"
        );
        let report = en_resume(&generated, &en_requirements());
        let hits = fired(&report, CONTENT_LANGUAGE_MISMATCH);
        assert_eq!(hits[0].section.as_deref(), Some(heading));
        assert_eq!(
            hits[0].severity,
            Severity::Warning,
            "{article} {name} heading has no SectionKey to repair — a Critical here \
             would be unclearable; got {hits:#?}"
        );
        assert!(
            report.ok,
            "a Warning alone must not block the report; got ok={}",
            report.ok
        );
    }
}

/// **The bug this whole fix closes, pinned as a permanent regression.**
/// Before this fix, [`section_language_issues`]'s gate checked ONLY
/// `is_language_mismatch` on the whole document — never whether the TARGET
/// language was corroborated by any witness at all — so a section could be
/// flagged against a `target_language` neither the job ad nor the
/// candidate's own source résumé ever vouched for. Same defect as
/// [`a_language_critical_needs_at_least_one_reliable_witness`] above, but at
/// SECTION scope instead of document scope: `EN_EXPERIENCE_DRIFTED_ITALIAN`
/// reads confidently English at the WHOLE-document level (premise below), an
/// unreliable job ad plus a genuinely French source corroborate nothing for
/// `"en"` (premise below), yet the EXPERIENCE section really is Italian —
/// before this fix, `section_language_issues` fired a Critical on it anyway,
/// because its old gate never asked whether `"en"` itself was credible.
///
/// Mutation check: drop the `!target_is_corroborated(...) ||` clause from
/// `section_language_issues` (restoring the old single-condition gate) and
/// this test goes red — `CONTENT_LANGUAGE_MISMATCH` fires, `Critical`,
/// `section: Some("EXPERIENCE")`.
#[test]
fn a_drifted_section_stays_quiet_when_the_target_language_is_not_corroborated() {
    let unreliable_ad = "Terraform AWS PostgreSQL Kubernetes platform engineer";
    assert!(
        !is_language_mismatch(EN_EXPERIENCE_DRIFTED_ITALIAN, "en"),
        "premise: the whole document must not confidently read as \
         non-English — otherwise this test would pass for the WRONG reason \
         (the is_language_mismatch half of the gate alone, not the \
         corroboration half this test targets)"
    );
    assert!(
        !document_language_mismatch(EN_WRONG_LANGUAGE, FR_RESUME, unreliable_ad, "en"),
        "premise: neither this unreliable ad nor a genuinely French source \
         corroborates \"en\" — proven indirectly via a text already known to \
         confidently read as non-English (EN_WRONG_LANGUAGE fires when \
         paired with EN_SOURCE/EN_JOB_AD elsewhere in this file), so a false \
         result here with the SAME generated text can only be explained by \
         the corroboration half of document_language_mismatch being false"
    );

    let report = report_for(EN_EXPERIENCE_DRIFTED_ITALIAN, FR_RESUME, unreliable_ad, &[]);
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}
