use super::*;

// -- real-world text shape: line-wrap + quote-prefix + NBSP -----------
//
// Every OTHER fixture in this file is artificially newline-free (a Rust
// `\`-continuation strips both the newline AND the leading indent into
// one long line) -- realistic in length while dodging the only property
// that actually matters for a substring match: whether the phrase stays
// CONTIGUOUS. Real mail hard-wraps at ~72-80 columns, and a quoted reply
// region prefixes each wrapped line with "> ". These tests build a
// genuinely wrapped, genuinely quoted body (real `\n`, not a literal) at
// several measured widths, so a wrap boundary landing inside the
// discriminating phrase is reproduced, not assumed.

/// Word-wrap `text` to at most `width` columns, breaking only at
/// existing spaces (never mid-word) -- mirrors how a real mail client
/// hard-wraps a plain-text body -- then prefixes every line with "> "
/// (one level of reply-quoting). A tiny test-only generator so the SAME
/// source paragraph is rendered at several realistic widths instead of
/// hand-typing wrapped text, which would silently depend on exactly the
/// column boundary a bug depends on.
fn wrap_quoted(text: &str, width: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate_len = if line.is_empty() {
            word.chars().count()
        } else {
            line.chars().count() + 1 + word.chars().count()
        };
        if !line.is_empty() && candidate_len > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
        .into_iter()
        .map(|l| format!("> {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The source paragraph every wrap-width test below renders -- a
/// realistic rejection body carrying the discriminating phrase, with
/// ONE regular space swapped for U+00A0 (NBSP) between "we" and "have"
/// (`mail-parser`'s `html_to_text` emits `&nbsp;` verbatim).
fn nbsp_rejection_paragraph() -> String {
    "Thank you for your interest in our team here at Acme Corp. After \
         further discussion among the panel, we\u{00A0}have decided not be \
         moving forward with your application at this time, though we were \
         impressed by your background. We wish you the very best in your \
         ongoing search."
        .to_string()
}

#[test]
fn wrapped_quoted_nbsp_body_classifies_as_rejection_at_every_measured_width() {
    for width in [72, 76, 78, 80] {
        let body = wrap_quoted(&nbsp_rejection_paragraph(), width);
        assert!(body.contains('\n'), "fixture must be genuinely multi-line");
        assert_eq!(
            classify_intent("Update", Some(&body)),
            Some(EmailIntent::Rejection),
            "wrapped at {width} columns"
        );
    }
}

#[test]
fn a_curly_apostrophe_still_matches_the_ascii_apostrophe_phrase() {
    // The corpus stores a straight apostrophe (') in e.g. the French
    // rejection phrase "n'a pas ete retenue"; real mail commonly sends
    // U+2019 (') instead. Body only (that phrase is Location::Body).
    //
    // Deliberately isolated to ONLY this phrase (no surrounding
    // boilerplate like "regrettons de vous informer", itself a
    // discriminating rejection phrase) -- an earlier version of this
    // test accidentally also matched that OTHER phrase and stayed green
    // with no apostrophe folding at all, proving nothing about the
    // apostrophe itself.
    let body = "Votre candidature n\u{2019}a pas \u{e9}t\u{e9} retenue.";
    assert_eq!(
        classify_intent("Update", Some(body)),
        Some(EmailIntent::Rejection)
    );
}

#[test]
fn nfd_decomposed_accents_still_match_the_nfc_written_phrase() {
    // The corpus phrase is written NFC (precomposed "é" = U+00E9). Some
    // mail clients / OS text layers instead emit NFD (decomposed: "e"
    // U+0065 + COMBINING ACUTE ACCENT U+0301) — a naive substring match
    // against a decomposed body misses every accented phrase (measured:
    // 16 of 16 accented discriminating rejection phrases). Spelled out
    // with explicit `\u{}` escapes for "été" (not a literal decomposed
    // character pasted into the source) so the decomposition is
    // unambiguous and can't silently re-compose under an editor's own
    // normalization. Same isolation lesson as the apostrophe test above
    // — no other discriminating phrase nearby.
    let body = "Votre candidature n'a pas e\u{0301}te\u{0301} retenue.";
    assert_eq!(
        classify_intent("Update", Some(body)),
        Some(EmailIntent::Rejection)
    );
}
