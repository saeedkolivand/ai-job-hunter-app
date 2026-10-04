//! Every named threshold pinned, its effect at the boundary, and the report-size caps.

use super::{support::*, *};

/// R4-F7 — [`issue`] clamped `message` and `evidence` but copied `section`
/// VERBATIM, and an ATX heading (`# …`) has no length rule of its own. 200
/// per-line issues each carrying a 1 KB heading blew the byte budget the cap
/// exists to keep, so the arithmetic in [`ISSUE_MESSAGE_MAX_BYTES`]' doc was
/// simply wrong.
#[test]
fn a_pathological_section_heading_is_clamped_like_the_message_and_evidence() {
    let heading = "Sonstige Tätigkeiten ".repeat(200); // ~4 KB, multibyte.
    let generated = format!(
        "# {heading}\n\n- {}\n",
        "x".repeat(ats::MAX_BULLET_CHARS + 50)
    );
    let report = report_against(&generated, EN_SOURCE);
    let section = fired(&report, ATS_LONG_BULLET)[0]
        .section
        .as_deref()
        .expect("a long bullet under a heading reports its section");
    assert!(
        section.len() <= ISSUE_SECTION_MAX_BYTES,
        "section must be clamped like every other issue field; got {} bytes",
        section.len()
    );
    assert!(
        section.ends_with('…'),
        "a clamped section must read as visibly cut; got {section:?}"
    );
}

/// M-3: an uncapped `issues` list can grow the serialized report past the
/// save path's `QUALITY_REPORT_MAX_BYTES` (256 KiB) clamp, which then
/// truncates it mid-JSON, fails to parse, and silently discards a fresh
/// report. `MAX_CONTENT_ISSUES` bounds the list at the source so that clamp
/// is never reached — proved here by pinning both the cap AND that a
/// pathological duplicate-bullet flood past it still: stays `ok` exactly
/// when it should, truncates to the cap plus one trailing marker, and
/// serializes comfortably under the byte clamp.
#[test]
fn oversized_issue_list_is_capped_with_a_visible_truncation_marker() {
    assert_eq!(MAX_CONTENT_ISSUES, 200);

    // Start from the CLEAN fixture (so every factual/structure check that
    // compares against the source résumé stays satisfied — zero Criticals),
    // then append one clique of 250 byte-identical bullets.
    // `duplicates::validate` marks every bullet after the first as "involved"
    // once it matches an earlier one, so a clique of k identical bullets
    // fires k-1 `duplicate.bullet` Warnings — 249 here, comfortably past the
    // cap, from only 250 extra bullets (well under
    // `duplicates::MAX_DUP_BULLETS` = 400, so this test stays orthogonal to
    // the M-2 fix).
    let mut generated = EN_CLEAN.to_string();
    for _ in 0..250 {
        generated
            .push_str("- Delivered feature rollout across the payments platform for the team\n");
    }

    let report = en_resume(&generated, &en_requirements());

    assert_eq!(
        report.issues.len(),
        MAX_CONTENT_ISSUES + 1,
        "must truncate to the cap plus exactly one trailing marker"
    );
    let marker = report.issues.last().expect("at least one issue");
    assert_eq!(marker.code, REPORT_TRUNCATED);
    assert_eq!(marker.severity, Severity::Warning);
    assert!(
        !marker.message.trim().is_empty() && marker.evidence.is_some(),
        "the marker itself must still be evidence-backed and advisory"
    );
    assert!(
        report.ok,
        "no Critical was ever produced here, so truncating warnings must not flip ok"
    );

    let bytes = serde_json::to_vec(&report).expect("report must serialize");
    assert!(
        bytes.len() < 256 * 1024,
        "capped report must stay comfortably under the save path's byte clamp, got {} bytes",
        bytes.len()
    );
}

/// M-3 bounds issue *count* ([`MAX_CONTENT_ISSUES`]); this pins that `issue()`
/// also bounds issue *size* — `message`/`evidence` clamp to
/// [`ISSUE_MESSAGE_MAX_BYTES`]/[`ISSUE_EVIDENCE_MAX_BYTES`], UTF-8
/// char-boundary safe even when the byte cut lands mid-character, and the
/// clamped result carries a visible `…` marker instead of silently losing its
/// tail. `é` is a 2-byte character and the cap (400) minus the marker's 3
/// bytes (397) is odd, so the cut provably lands mid-character — proving the
/// clamp walks back to a real boundary instead of splitting one.
#[test]
fn issue_text_fields_are_clamped_to_their_byte_caps() {
    let long_message = "é".repeat(300); // 600 bytes, well past the 400-byte cap
    let long_evidence = "é".repeat(300);
    let long_section = "é".repeat(300);
    assert!(long_message.len() > ISSUE_MESSAGE_MAX_BYTES);
    assert!(long_evidence.len() > ISSUE_EVIDENCE_MAX_BYTES);
    assert!(long_section.len() > ISSUE_SECTION_MAX_BYTES);

    let built = issue(
        ATS_LONG_BULLET,
        Some(&long_section),
        long_message.clone(),
        Some(long_evidence.clone()),
    );

    // `section` is a heading copied out of the generated document, so it is
    // exactly as untrusted as the other two — and an ATX heading has no length
    // rule of its own anywhere in the parser.
    let section = built
        .section
        .clone()
        .expect("section must survive clamping");
    assert!(
        section.len() <= ISSUE_SECTION_MAX_BYTES,
        "section must be clamped to the cap, got {} bytes",
        section.len()
    );
    assert!(
        section.ends_with('…'),
        "a clamped section must carry the truncation marker"
    );
    assert!(
        long_section.starts_with(section.trim_end_matches('…')),
        "the clamped section must be an exact, unbroken prefix of the original"
    );

    assert!(
        built.message.len() <= ISSUE_MESSAGE_MAX_BYTES,
        "message must be clamped to the cap, got {} bytes",
        built.message.len()
    );
    assert!(
        built.message.ends_with('…'),
        "a clamped message must carry the truncation marker"
    );
    let message_prefix = built.message.trim_end_matches('…');
    assert!(
        long_message.starts_with(message_prefix),
        "the clamped message must be an exact, unbroken prefix of the original"
    );

    let evidence = built.evidence.expect("evidence must survive clamping");
    assert!(
        evidence.len() <= ISSUE_EVIDENCE_MAX_BYTES,
        "evidence must be clamped to the cap, got {} bytes",
        evidence.len()
    );
    assert!(
        evidence.ends_with('…'),
        "a clamped evidence span must carry the truncation marker"
    );
    let evidence_prefix = evidence.trim_end_matches('…');
    assert!(
        long_evidence.starts_with(evidence_prefix),
        "the clamped evidence must be an exact, unbroken prefix of the original"
    );
}

/// The reviewer's exact failure shape: `ats.long_bullet` fires *because* a
/// bullet is long, and used to quote it verbatim as `evidence` — so a single
/// pathological (but count-legal, well under `MAX_CONTENT_ISSUES`) bullet
/// could still blow the byte clamp on its own. Pins that the per-issue clamp
/// in `issue()` catches what the count cap (M-3) cannot.
#[test]
fn long_bullet_evidence_is_clamped_even_though_the_bullet_is_long() {
    let long_bullet = format!("- {}\n", "word ".repeat(400)); // ~2KB bullet
    assert!(long_bullet.len() > 2000);

    let mut generated = EN_CLEAN.to_string();
    generated.push_str(&long_bullet);

    let report = en_resume(&generated, &en_requirements());
    let hits = fired(&report, ATS_LONG_BULLET);

    for issue in hits {
        let evidence = issue
            .evidence
            .as_ref()
            .expect("ats.long_bullet must carry the offending bullet as evidence");
        assert!(
            evidence.len() <= ISSUE_EVIDENCE_MAX_BYTES,
            "evidence for a {}-byte bullet must still clamp to the cap, got {} bytes",
            long_bullet.len(),
            evidence.len()
        );
    }
}

// Each const gets a test. Loosening a threshold has to be a deliberate edit to
// a named expectation, not a one-character change nobody reviews.

#[test]
fn duplicate_threshold_is_pinned() {
    assert_eq!(duplicates::DUPLICATE_JACCARD_THRESHOLD, 0.8);
    assert_eq!(duplicates::MIN_TOKENS_FOR_DUPLICATE, 4);
    assert_eq!(duplicates::MAX_DUP_BULLETS, 400);
}

/// M-2: the near-duplicate scan is O(bullets²) — before [`duplicates::MAX_DUP_BULLETS`],
/// a malformed/hostile document with thousands of bullet-shaped lines held a
/// tokio worker for seconds on every save. A pathological bullet count must
/// both (a) return fast and (b) still flag a genuine duplicate placed well
/// inside the cap — the cap must never silently disable the check entirely.
#[test]
fn pathological_bullet_count_returns_quickly_and_still_flags_a_duplicate_within_the_cap() {
    let mut generated =
        String::from("Jane Doe\n\nEXPERIENCE\n\nEngineer | Acme | 2020 - Present\n");
    // A genuine duplicate pair, placed at the very start — well inside
    // MAX_DUP_BULLETS regardless of how many filler bullets follow.
    generated.push_str(
        "- Cut checkout latency from 480ms to 90ms with a Redis cache in front of the ledger\n",
    );
    generated.push_str(
        "- Cut checkout latency from 480ms to 90ms with a Redis cache in front of the ledger service\n",
    );
    // Far more bullets than the cap — a real résumé never has anywhere close
    // to this many.
    for i in 0..6_000 {
        generated.push_str(&format!(
            "- Delivered feature rollout {i} across the payments platform for the {i} team\n"
        ));
    }

    let input = content_input(&generated, EN_SOURCE, EN_JOB_AD);
    let ctx = Analysis::new(&input);

    let start = std::time::Instant::now();
    let (issues, ratio) = duplicates::validate(&ctx);
    let elapsed = start.elapsed();

    // The bound discriminates capped from uncapped, not fast from slow: capped,
    // the scan is linear tokenization + a bounded pairwise pass (~2.2s worst
    // observed on a slow shared CI runner in a debug build); uncapped, 6,000
    // bullets is ~18M pairwise comparisons — tens of seconds anywhere. 500ms
    // held locally but failed every CI retry, blocking merges on runner speed.
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "the pairwise duplicate scan must stay bounded past MAX_DUP_BULLETS bullets, \
         took {elapsed:?}"
    );
    assert!(
        issues.iter().any(|i| i.code == DUPLICATE_BULLET),
        "a genuine duplicate well inside the cap must still be flagged"
    );
    assert!(ratio > 0.0, "duplicateRatio must reflect the flagged pair");
}

/// `MIN_TOKENS_FOR_DENSITY` moved 50 -> 75 when the keyword kernel became
/// language-aware. Recorded here rather than silently re-pinned: filtering a
/// posting's own function words makes documents ~18% shorter in CONTENT tokens
/// (a real German fixture, 89 -> 73 raw), which tipped an honest `backend` x3
/// from 3.66% to 4.17% and read as keyword stuffing. 75 is derived — `3/total >
/// 0.04` holds for every `total < 75` — not fitted to that one fixture. The
/// `eval` corpus is what catches the regression; this pin is what makes moving
/// the number deliberate.
#[test]
fn ats_thresholds_are_pinned() {
    assert_eq!(ats::MAX_KEYWORD_DENSITY_RATIO, 0.04);
    assert_eq!(ats::MAX_KEYWORD_OCCURRENCES, 6);
    assert_eq!(ats::MIN_TOKENS_FOR_DENSITY, 75);
    assert_eq!(ats::MAX_BULLET_CHARS, 200);
    assert_eq!(ats::MIN_BULLETS_PER_ROLE, 1);
    assert_eq!(ats::MAX_BULLETS_PER_ROLE, 6);
}

#[test]
fn voice_thresholds_are_pinned() {
    assert_eq!(voice::MIN_SENTENCES_FOR_BURSTINESS, 8);
    assert_eq!(voice::MIN_SENTENCE_LENGTH_STDDEV, 4.0);
    assert_eq!(voice::MAX_TRIPLETS_PER_TEN_SENTENCES, 2.0);
    assert_eq!(voice::EM_DASH_WORDS_PER_ALLOWED, 150);
    assert_eq!(voice::TEMPLATE_OPENER_SCAN_CHARS, 200);
    assert_eq!(voice::MIN_JOB_SPECIFIC_TOKENS_IN_LETTER, 2);
}

#[test]
fn factual_and_alignment_thresholds_are_pinned() {
    assert_eq!(factual::MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS, 4);
    // The asymmetry is load-bearing: raising this to the distinctive bar is
    // what turned "IBM Deutschland GmbH" written as "IBM" into a Critical.
    assert_eq!(factual::MIN_SURVIVAL_COMPANY_TOKEN_CHARS, 2);
    assert_eq!(factual::MIN_WORDS_IN_LETTER_BODY_LINE, 8);
    // Shared with the marker-less arm of `HEADER_PHONE_RE` on purpose — the two
    // "is this a phone?" answers may differ in SHAPE, never in length.
    assert_eq!(factual::MIN_BARE_PHONE_LINE_DIGITS, 7);
    assert_eq!(alignment::TOP_REQUIREMENT_MATCH_RATIO, 0.5);
    assert_eq!(alignment::MIN_COVERAGE_DROP_POINTS, 5.0);
    assert_eq!(consistency::MAX_PROJECT_DESCRIPTION_LINES, 3);
    assert_eq!(consistency::MAX_SKILLS_LABEL_WORDS, 3);
    assert_eq!(MIN_CHARS_FOR_LANGUAGE_CHECK, 120);
    // Confirmation-review finding 3: the 0.2 threshold was asserted nowhere.
    // One word in five opening lowercase is enough to read a section as prose.
    assert_eq!(PROSE_LOWERCASE_WORD_RATIO, 0.2);
}

/// The duplicate threshold must actually BITE at its boundary: a pair just
/// under 0.8 is not a duplicate, a pair at or over it is. Pinning the number
/// without pinning its effect is how a threshold silently stops working.
#[test]
fn duplicate_threshold_boundary_behaves() {
    let a: std::collections::HashSet<String> = ["one", "two", "three", "four", "five"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    // 4 shared of 6 union = 0.666 — under the threshold.
    let under: std::collections::HashSet<String> = ["one", "two", "three", "four", "six"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert!(jaccard(&a, &under) < duplicates::DUPLICATE_JACCARD_THRESHOLD);
    // Identical sets = 1.0 — over it.
    assert!(jaccard(&a, &a) >= duplicates::DUPLICATE_JACCARD_THRESHOLD);
    // Empty vs empty must be 0.0, never "identical".
    assert_eq!(
        jaccard(
            &std::collections::HashSet::new(),
            &std::collections::HashSet::new()
        ),
        0.0
    );
}

#[test]
fn contains_phrase_respects_word_boundaries() {
    assert!(contains_phrase("this is vital work", "vital"));
    assert!(
        !contains_phrase("we revitalized the pipeline", "vital"),
        "a substring inside a longer word must not fire"
    );
    assert!(contains_phrase("not just faster, but cheaper", "not just"));
    assert!(!contains_phrase("", "vital"));
    assert!(!contains_phrase("anything", ""));
}

#[test]
fn language_normalization_matches_the_prompt_side() {
    assert_eq!(normalize_language("de-DE"), "de");
    assert_eq!(normalize_language("EN"), "en");
    assert_eq!(normalize_language(""), "en");
    assert_eq!(normalize_language("   "), "en");
}

/// L-3: a control character (a raw newline in particular) must never survive
/// into the 2-char result — it would otherwise reach `ctx.lang`, and from
/// there this module's own `validate:content` span text and a
/// `content.language_mismatch` issue's `evidence`, a log-injection
/// primitive. `.trim()` alone does NOT catch this: it only strips
/// leading/trailing whitespace, and a newline in the MIDDLE of the string
/// survives trim untouched.
#[test]
fn language_normalization_strips_control_characters() {
    assert_eq!(
        normalize_language("a\nb"),
        "ab",
        "an internal control character must never reach the normalized code"
    );
    assert!(!normalize_language("a\nb").contains('\n'));
    assert_eq!(normalize_language("\t\u{7}de"), "de");
    // A wholly-control-character input degrades to the same "en" default an
    // empty string gets — never an empty-but-not-quite string.
    assert_eq!(normalize_language("\n\n"), "en");
}

/// The credential family's thresholds, pinned like every other named `const` in
/// this module. A silently-loosened threshold is how a validator stops
/// validating — and NONE of these eight bounds the sole CRITICAL code path
/// (`factual.unsourced_certification`'s acronym arm, gated by the curated
/// 23-entry list and a word-boundary check, not a numeric threshold): all
/// eight bound a WARNING instead — tenure, or the certification issuer-phrase
/// arm.
#[test]
fn credential_thresholds_are_pinned() {
    // One year, because a year column carries no months: `2018 - 2021` is
    // anywhere from 24 to 47 months, and one year is exactly the amount by
    // which subtracting the two numbers can understate the truth.
    assert_eq!(credentials::CAREER_SPAN_SLACK_YEARS, 1);
    assert_eq!(credentials::CLAIM_CONTEXT_CHARS, 40);
    assert_eq!(credentials::MAX_PLAUSIBLE_TENURE_YEARS, 60);
    assert_eq!(credentials::CERT_ISSUER_WINDOW_CHARS, 60);
    assert_eq!(credentials::CERT_EVIDENCE_LINE_CHARS, 120);
    assert_eq!(credentials::SPAN_TAIL_CHARS, 16);
    assert_eq!(credentials::CERT_ROLE_NOUN_WINDOW_TOKENS, 2);
    assert_eq!(credentials::TENURE_SUBJECT_TOKENS, 2);
}
