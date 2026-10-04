use super::*;

/// One discriminating phrase per language per intent. A shared loop must not let one case's
/// failure hide another's (a mutation that only breaks ONE case has to be visible), so every
/// mismatch is collected and reported together instead of short-circuiting on the first.
#[test]
fn classifies_each_language_and_intent() {
    use EmailIntent as I;
    // German has NO discriminating confirmation phrase in the surviving
    // corpus (`"wir haben ihre bewerbung"` is the only `de` confirmation
    // entry and it is `discriminating: false`) — a real, documented gap, not
    // an oversight. See `de_confirmation_phrase_alone_is_not_enough_to_decide`
    // below, which pins exactly this. NOT a German-only gap, either — see
    // the module doc's "Recall gap, by intent, not just by language"
    // section: English itself has only ONE discriminating confirmation
    // phrase and ONE discriminating offer phrase.
    // Body-only phrases are tried under a neutral subject; subject-only phrases (`location:
    // subject`) with no body at all.
    let body_cases: &[(&str, EmailIntent)] = &[
        ("if you are among qualified candidates", I::Confirmation), // en
        ("invite you for a job interview", I::Interview),           // en
        ("having you on our team", I::Offer),                       // en
        ("move forward with other candidates", I::Rejection),       // en
        ("virtuellen vorstellungsgespräch", I::Interview),          // de
        ("zusage zu ihrer bewerbung", I::Offer),                    // de
        ("andere besetzung", I::Rejection),                         // de
        ("bonne réception de ta candidature", I::Confirmation),     // fr
        ("ne donnerons pas suite", I::Rejection),                   // fr
        ("entusiasmados de ofrecerte", I::Offer),                   // es
        ("no proceder con tu candidatura", I::Rejection),           // es
        ("la sua candidatura è arrivata", I::Confirmation),         // it
        ("lettera di impegno all'assunzione", I::Offer),            // it
        ("non è risultata prescelta", I::Rejection),                // it
        ("sollicitatie is binnen", I::Confirmation),                // nl
        ("telefonisch gesprek van 30 minuten", I::Interview),       // nl
        ("bieden je graag de functie", I::Offer),                   // nl
        ("niet verder mee te nemen", I::Rejection),                 // nl
        ("queremos marcar uma entrevista", I::Interview),           // pt
        ("parabéns pela proposta", I::Offer),                       // pt
        ("não seguiremos o processo com você", I::Rejection),       // pt
    ];
    let subject_cases: &[(&str, EmailIntent)] = &[
        ("convocation à un entretien", I::Interview),     // fr
        ("acceptation de candidature", I::Offer),         // fr
        ("currículum recibido", I::Confirmation),         // es
        ("invitación para una entrevista", I::Interview), // es
        ("convocazione a colloquio", I::Interview),       // it
        ("confirmação de candidatura", I::Confirmation),  // pt
    ];
    let body = body_cases
        .iter()
        .map(|&(body, expected)| ("Update", Some(body), expected));
    let subject = subject_cases
        .iter()
        .map(|&(subject, expected)| (subject, None, expected));
    let mismatches: Vec<String> = body
        .chain(subject)
        .filter_map(|(subject, body, expected)| {
            let got = classify_intent(subject, body);
            (got != Some(expected))
                .then(|| format!("{subject:?} / {body:?}: expected {expected:?}, got {got:?}"))
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

// ── rule 3: a non-discriminating phrase never decides an intent alone ──

#[test]
fn en_non_discriminating_confirmation_phrase_alone_decides_nothing() {
    assert_eq!(
        classify_intent("Update", Some("you will hear from us")),
        None
    );
}

#[test]
fn de_confirmation_phrase_alone_is_not_enough_to_decide() {
    // The only `de` confirmation entry in the surviving corpus is
    // `discriminating: false` — this is exactly why: it must not decide
    // anything on its own, so German confirmations currently classify
    // as `None` rather than `Some(Confirmation)`. A real, documented
    // corpus gap (see the module-level doc), not a bug in this rule.
    assert_eq!(
        classify_intent("Update", Some("wir haben ihre bewerbung")),
        None
    );
}

// ── rule 5: `location` is a constraint, not a hint ──────────────────────

#[test]
fn subject_only_phrase_does_not_fire_from_the_body() {
    // "convocation à un entretien" is Location::Subject.
    assert_eq!(
        classify_intent("Update", Some("convocation à un entretien")),
        None
    );
}

#[test]
fn body_only_phrase_does_not_fire_from_the_subject() {
    // "move forward with other candidates" is Location::Body.
    assert_eq!(
        classify_intent("move forward with other candidates", None),
        None
    );
}

#[test]
fn both_location_phrase_fires_from_the_subject_alone() {
    assert_eq!(
        classify_intent("not be moving forward with your application", None),
        Some(EmailIntent::Rejection)
    );
}

#[test]
fn both_location_phrase_fires_from_the_body_alone() {
    assert_eq!(
        classify_intent(
            "Update",
            Some("not be moving forward with your application")
        ),
        Some(EmailIntent::Rejection)
    );
}

// ── rule 1: negation lives INSIDE the phrase, never inferred ────────────

#[test]
fn positive_phrasing_without_the_negation_does_not_trigger_rejection() {
    // "moving forward with your application" alone (no "not") must not
    // fire the "not be moving forward with your application" rejection
    // phrase — a substring match against the FULL negated phrase can't
    // be fooled by its own positive tail.
    assert_eq!(
        classify_intent(
            "Next steps",
            Some("we are excited that you'll be moving forward with your application")
        ),
        None
    );
}

// ── rule 4: rejection wins whenever it fires alongside anything else ───

#[test]
fn rejection_wins_when_a_confirmation_phrase_and_a_rejection_phrase_both_fire() {
    // The exact real-world thread-reuse scenario this whole slice exists
    // for: a rejection whose body still carries an earlier confirmation
    // line (quoted reply, or a template that opens with a receipt
    // acknowledgement before the bad news).
    let body = "if you are among qualified candidates we will follow up. unfortunately, \
                     we have decided not be moving forward with your application at this time.";
    assert_eq!(
        classify_intent("Your application to Acme Corp", Some(body)),
        Some(EmailIntent::Rejection)
    );
}

#[test]
fn rejection_wins_over_interview_when_both_fire() {
    let body = "invite you for a job interview — actually, we have decided to move forward \
                     with other candidates instead.";
    assert_eq!(
        classify_intent("Update", Some(body)),
        Some(EmailIntent::Rejection)
    );
}

#[test]
fn rejection_wins_over_offer_when_both_fire() {
    let body = "having you on our team would have been great, but we have decided to move \
                     forward with other candidates.";
    assert_eq!(
        classify_intent("Update", Some(body)),
        Some(EmailIntent::Rejection)
    );
}

// ── ladder tie-break among the 3 non-rejection intents ──────────────────

#[test]
fn offer_beats_interview_when_both_fire_without_rejection() {
    let body = "having you on our team — following up on your invite you for a job interview \
                     last week.";
    assert_eq!(
        classify_intent("Update", Some(body)),
        Some(EmailIntent::Offer)
    );
}

#[test]
fn interview_beats_confirmation_when_both_fire_without_rejection() {
    let body = "if you are among qualified candidates, and — good news — we'd like to invite you \
             for a job interview.";
    assert_eq!(
        classify_intent("Update", Some(body)),
        Some(EmailIntent::Interview)
    );
}

// ── totality / bounded input ────────────────────────────────────────────

#[test]
fn empty_subject_and_no_body_classifies_as_no_intent_without_panicking() {
    assert_eq!(classify_intent("", None), None);
}

#[test]
fn a_discriminating_phrase_past_intent_scan_bytes_is_still_not_matched() {
    // A cap still exists — just a far more generous one than the
    // fingerprint snippet. The padding length is a PINNED LITERAL, not
    // `INTENT_SCAN_BYTES` itself: deriving the padding from the same
    // constant it is meant to test makes the test self-referential — a
    // regression that bumps the constant to (say) 10 MB would bump this
    // padding to match and stay green, proving the cap exists while the
    // cap itself silently grew unbounded. A fixed 20_000 catches that: if
    // `INTENT_SCAN_BYTES` is ever raised, this phrase (just past the
    // OLD, pinned bound) becomes visible again and the assertion fails.
    //
    // This literal must be updated by hand if `INTENT_SCAN_BYTES` is
    // ever deliberately raised — that friction is the point.
    const PINNED_SCAN_BOUND_BYTES: usize = 20_000;
    let padding = "x".repeat(PINNED_SCAN_BOUND_BYTES);
    let body = format!("{padding} move forward with other candidates");
    assert_eq!(classify_intent("Update", Some(&body)), None);
}

#[test]
fn a_realistic_rejection_body_past_the_old_500_byte_mark_classifies_as_rejection() {
    // A realistic full ATS rejection: a greeting plus "thank you for
    // applying"/volume-of-applicants boilerplate pushes the actual
    // discriminating phrase well past the OLD 500-byte
    // `BODY_SNIPPET_BYTES` mark — exactly the shape that was silently
    // missed before `INTENT_SCAN_BYTES` replaced it as this module's
    // body-scan bound.
    let body = "Dear Applicant,\n\n\
            Thank you so much for taking the time to apply for the Senior Backend Engineer \
            position at Acme Corp, and for your patience throughout our review process. We \
            received a very large number of applications for this role, and our hiring team \
            carefully reviewed every candidate's background, skills, and experience against \
            what the position required. This was one of the most competitive searches we have \
            run this year, and choosing among so many strong candidates was genuinely difficult \
            for the whole panel.\n\n\
            After careful consideration, we have decided not be moving forward with your \
            application at this time.\n\n\
            We will keep your resume on file for six months in case a better-matching role \
            opens up, and we wish you the very best in your job search. Thank you again for \
            your interest in Acme Corp.\n\n\
            Best regards,\nThe Acme Corp Talent Acquisition Team";
    assert!(
        body.len() > 500,
        "test fixture must actually exceed the old 500-byte cap to be meaningful"
    );
    assert_eq!(
        classify_intent("Your application to Acme Corp", Some(body)),
        Some(EmailIntent::Rejection)
    );
}

#[test]
fn unrelated_text_returns_no_intent() {
    assert_eq!(
        classify_intent(
            "Your weekly newsletter",
            Some("Enjoy this week's roundup of articles.")
        ),
        None
    );
}

// ── wider body window: reasoning about what a bigger scan lets in ───────

#[test]
fn rejection_still_wins_when_a_stale_quoted_confirmation_phrase_sits_past_the_old_cap() {
    // The exact concern a wider window raises: MORE quoted, earlier
    // thread content is now visible. Here a confirmation phrase from an
    // earlier message in the thread sits well past the OLD 500-byte cap
    // (which would previously have hidden it entirely). Rejection still
    // wins: the priority check has no positional/order dependence — it
    // just asks "does ANY rejection phrase match anywhere" — independent
    // of where in the (now wider) window a competing intent's phrase
    // also happens to sit.
    let padding = "quoted earlier thread content ".repeat(20);
    assert!(padding.len() > 500);
    let body = format!(
        "we have decided not be moving forward with your application. {padding} \
             if you are among qualified candidates we will be in touch."
    );
    assert_eq!(
        classify_intent("Update", Some(&body)),
        Some(EmailIntent::Rejection)
    );
}

#[test]
fn known_precision_limit_a_stale_quoted_offer_phrase_can_beat_a_current_interview_phrase() {
    // Documents a REAL, accepted-not-fixed limitation the wider window
    // introduces — unlike rejection (which always wins regardless of
    // position), the ladder tie-break among the non-rejection three
    // (`Offer` > `Interview` > `Confirmation`) has no positional
    // awareness either. A stale "having you on our team" offer line
    // quoted from an OLDER message in the thread — now visible because
    // the window is wider — outranks a genuinely CURRENT
    // interview-scheduling phrase, even though the offer phrase isn't
    // about the current email at all. Flagged for the coordinator, not
    // silently accepted or fixed in this slice.
    let padding = "quoted earlier thread content ".repeat(20);
    assert!(padding.len() > 500);
    let body = format!(
        "invite you for a job interview next Tuesday. {padding} having you on our team \
             would have been great."
    );
    assert_eq!(
        classify_intent("Update", Some(&body)),
        Some(EmailIntent::Offer)
    );
}

// ── rule 2: cross-language collisions don't matter (no language check) ──

#[test]
fn mixed_language_rejection_phrases_still_classify_as_rejection() {
    let body = "move forward with other candidates -- andere besetzung.";
    assert_eq!(
        classify_intent("Update", Some(body)),
        Some(EmailIntent::Rejection)
    );
}
