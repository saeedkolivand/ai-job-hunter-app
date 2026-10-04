use super::*;

/// Every well-formed shape the tokeniser must still authorise — a gate that refuses every
/// legitimate email is as broken as one that lets attackers through. Each header must yield
/// `("pass", "greenhouse.io")`; the label names the shape (it is the former test's name).
#[test]
fn legitimate_shapes_authorise() {
    let cases: &[(&str, &str)] = &[
        (
            "legitimate_trailing_semicolon_with_nothing_after_still_authorises",
            "mx.google.com; dmarc=pass header.from=greenhouse.io;",
        ),
        (
            "legitimate_trailing_semicolon_with_whitespace_still_authorises",
            "mx.google.com; dmarc=pass header.from=greenhouse.io; ",
        ),
        (
            "legitimate_trailing_semicolon_then_a_comment_still_authorises",
            "mx.google.com; dmarc=pass header.from=greenhouse.io; (trailing note, no more sections)",
        ),
        // Yahoo's documented shape: `dmarc=pass(p=REJECT)`, no space
        // between the result and the parenthesized comment.
        (
            "legitimate_yahoo_no_space_before_the_comment_still_authorises",
            "mtaX.mail.gq1.yahoo.com; dmarc=pass(p=REJECT) header.from=greenhouse.io",
        ),
        // Fastmail-style tail: an `arc=none` section (no properties at
        // all), and a dmarc section carrying non-standard `policy.*`/
        // `x-*`-prefixed properties alongside the real `header.from=`.
        // None of these must interfere with finding the real verdict.
        (
            "legitimate_fastmail_shaped_tail_with_arc_and_policy_and_x_prefixed_properties",
            "in1-smtp.messagingengine.com; \
                       arc=none; \
                       dmarc=pass policy.published-domain=greenhouse.io policy.applied-disposition=none \
                       header.from=greenhouse.io x-spam-score=0.0",
        ),
        (
            "duplicate_header_from_within_one_section_agreeing_is_fine",
            "mx.google.com; dmarc=pass header.from=greenhouse.io header.from=greenhouse.io",
        ),
        // `\)` inside a comment is a quoted-pair (escapes the character,
        // does not affect nesting depth) — must not be read as the REAL
        // closing paren, which would leave the scanner desynced for
        // everything after.
        (
            "an_escaped_close_paren_inside_a_comment_does_not_end_it_early",
            "mx.google.com; spf=pass (a \\) literal paren) smtp.mailfrom=bounce@greenhouse.io; \
                       dmarc=pass header.from=greenhouse.io",
        ),
        // Confirms the char-based (never byte-offset) design handles
        // arbitrary multi-byte UTF-8 inside a value without corrupting
        // the surrounding structure — an emoji and an accented character,
        // both multi-byte in UTF-8, embedded in an otherwise-irrelevant
        // quoted property value ahead of the real dmarc section.
        (
            "multibyte_content_inside_a_quoted_pvalue_is_read_correctly",
            "mx.google.com; dkim=pass header.i=\"café 🎉 note\"; \
                       dmarc=pass header.from=greenhouse.io",
        ),
        (
            "nested_comments_are_skipped_as_one_unit",
            "mx.google.com; spf=pass (outer (inner (deepest) still inner) still outer) \
                       smtp.mailfrom=bounce@greenhouse.io; \
                       dmarc=pass header.from=greenhouse.io",
        ),
        (
            "an_escaped_quote_inside_a_quoted_string_does_not_end_it_early: an escaped quote inside the DKIM quoted-string must not be read as its closing quote",
            "mx.google.com; dkim=pass header.i=\"a \\\" quote\"; \
                       dmarc=pass header.from=greenhouse.io",
        ),
        (
            "a_quoted_header_from_pvalue_is_read_correctly",
            "mx.google.com; dmarc=pass header.from=\"greenhouse.io\"",
        ),
        // The addr-spec shape (`smtp.mailfrom="local part"@domain`) that
        // the envelope-injection exploit relies on — verified directly
        // here (not just via the end-to-end exploit test above): the
        // scanner must land correctly on the FOLLOWING `;`, not get
        // confused by the unquoted `@domain` glued onto the quoted part.
        (
            "a_quoted_local_part_glued_to_an_unquoted_domain_is_consumed_as_one_pvalue",
            "mx.google.com; spf=pass smtp.mailfrom=\"quoted local part\"@attacker.example; \
                       dmarc=pass header.from=greenhouse.io",
        ),
        // LOW fix (comment correction, not behaviour): pins that an empty
        // RESULT (`dmarc=;`) vanishes unread rather than being captured and
        // failing an equality check — see the comment on `process_section`'s
        // `let result = sc.read_value()?;` line for the full mechanism. This
        // is documented, intended behaviour, not a bug: the empty-result
        // section never reaches `found`, so it cannot corrupt the genuine
        // section that follows it.
        (
            "an_empty_dmarc_result_vanishes_unread_and_does_not_block_a_later_genuine_pass",
            "mx.google.com; dmarc=; dmarc=pass header.from=greenhouse.io",
        ),
        (
            "two_agreeing_dmarc_sections_authorise",
            "mx.google.com; \
                       dmarc=pass header.from=greenhouse.io; \
                       dmarc=pass header.from=greenhouse.io",
        ),
        (
            "method_version_is_tolerated",
            "mx.google.com; dmarc/1=pass header.from=greenhouse.io",
        ),
        (
            "microsoft_shaped_header_with_no_leading_authserv_id_parses: a leading authserv-id must not be REQUIRED to find the dmarc section",
            "spf=pass (sender IP is 40.107.1.1) smtp.mailfrom=greenhouse.io; \
                       dkim=pass (signature was verified) header.d=greenhouse.io; \
                       dmarc=pass action=none header.from=greenhouse.io; \
                       compauth=pass reason=100",
        ),
        (
            "a_realistic_gmail_pass_header_parses",
            "mx.google.com; \
                       dkim=pass header.i=@greenhouse.io header.s=selector header.b=abc123; \
                       spf=pass (google.com: domain of bounce@greenhouse.io designates 1.2.3.4 as permitted sender) smtp.mailfrom=bounce@greenhouse.io; \
                       dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=greenhouse.io",
        ),
    ];
    for (label, header) in cases {
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string())),
            "{label}"
        );
    }
}
