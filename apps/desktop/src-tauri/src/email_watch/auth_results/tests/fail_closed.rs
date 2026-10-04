use super::*;

// ── CRITICAL fix: truncation-on-empty-token must never return a ────
// ── STALE earlier verdict instead of failing closed ─────────────────

#[test]
fn truncation_attack_double_semicolon_does_not_return_a_stale_earlier_verdict() {
    // The exact shape from the finding: a genuine `dmarc=pass` section
    // for one domain, then a stray `;;` BEFORE the real, later,
    // disagreeing `dmarc=fail` section. The old `break`-on-empty-token
    // would stop here and return the FIRST (stale) verdict; must now
    // fail closed instead.
    let header = "mx.google.com; dmarc=pass header.from=victim.io;; \
                       dmarc=fail header.from=attacker.example";
    assert_eq!(
        dmarc_verdict(header),
        None,
        "a stray `;;` must fail closed, never return the verdict seen before it"
    );
}

#[test]
fn truncation_attack_a_stray_delimiter_after_a_semicolon_fails_closed() {
    // Each of `"` `=` `.` `/` `)` landing right after a top-level `;` used to stop the parse early
    // (an empty token read as end-of-input) and return the stale earlier verdict.
    for stray in ['"', '=', '.', '/', ')'] {
        let header = format!(
            "mx.google.com; dmarc=pass header.from=victim.io; {stray} \
                 dmarc=fail header.from=attacker.example"
        );
        assert_eq!(dmarc_verdict(&header), None, "stray {stray:?}");
    }
}

#[test]
fn truncation_attack_real_delivery_primitive_unescaped_close_paren_in_a_quoted_local_part() {
    // The reviewer's actual delivery mechanism, not just the abstract
    // `;;` shape above: RFC 5321 permits `)` unescaped inside a
    // QUOTED local-part (it only needs escaping inside a COMMENT, a
    // completely different context) -- so an attacker's envelope
    // local part `"a) ; dmarc=pass header.from=greenhouse.io ;;"`
    // genuinely, correctly closes the EARLIER SPF section's comment
    // early the moment `skip_comment` reaches that unescaped `)`
    // (comments give `"` no special meaning at all, per RFC 5322
    // `ccontent` -- so the fact this text was "inside quotes" from an
    // addr-spec point of view means nothing to a comment scanner).
    // Everything after that point parses as GENUINE grammar: a forged
    // `dmarc=pass header.from=greenhouse.io` section, then a stray
    // `;;` landing right before the REAL, later, genuine
    // `dmarc=fail header.from=attacker.example` section that Gmail
    // actually stamped. Must fail closed, not authorise the forgery.
    let header = concat!(
        "mx.google.com; ",
        "dkim=pass header.i=@attacker.example header.s=selector header.b=xyz789; ",
        "spf=pass (google.com: domain of \"a) ; dmarc=pass header.from=greenhouse.io ;;\"@attacker.example designates 5.6.7.8 as permitted sender) smtp.mailfrom=\"a) ; dmarc=pass header.from=greenhouse.io ;;\"@attacker.example; ",
        "dmarc=fail header.from=attacker.example"
    );
    assert_eq!(
        dmarc_verdict(header),
        None,
        "the forged dmarc=pass section (reachable only via the comment closing early) \
             must never authorise -- fail closed, do not fall back to it"
    );
}

// ── MEDIUM fix: a duplicate header.from WITHIN one section must be ──
// ── symmetric with the cross-section disagreement rule ──────────────

#[test]
fn duplicate_header_from_within_one_section_disagreeing_fails_closed() {
    let header = "mx.google.com; dmarc=pass header.from=greenhouse.io header.from=attacker.example";
    assert_eq!(
        dmarc_verdict(header),
        None,
        "two disagreeing header.from properties in ONE section must never silently pick one \
             — mirrors the cross-section disagreement rule"
    );
}

// ── reviewer's own most-plausible-miss: a top-level ';' produced by ──
// ── DECODING an already-received property value, not by any comment ──
// ── or quoted-string escape this module's own grammar handles ────────

#[test]
fn a_decoded_semicolon_inside_an_unquoted_property_value_still_fails_closed() {
    // The candidate raised: a DKIM `i=` tag is dkim-quoted-printable
    // encoded (RFC 6376 §2.11) in the DKIM-Signature header itself
    // (so a literal `;` there is always written `=3B`, never raw) —
    // but IF a verifying server decoded it back to a literal `;`
    // before echoing it, UNQUOTED, into this header's own
    // `header.i=` property, that would hand an attacker a top-level
    // `;` this module's grammar never sees coming from a comment or a
    // quoted-string. THIS CODEBASE never reaches that decode itself:
    // the IMAP fetch this module's caller performs
    // (`imap_client::fetch_headers_since`) requests exactly
    // `FROM SUBJECT DATE MESSAGE-ID AUTHENTICATION-RESULTS` — the
    // DKIM-Signature header is never fetched, let alone decoded, by
    // this crate. Whether some real mail provider's OWN
    // Authentication-Results-stamping code does this decode-then-embed
    // internally is a question about infrastructure this crate cannot
    // observe or verify.
    //
    // Untestable-as-a-real-exploit does not mean untested: this proves
    // the SHAPE (an unquoted, unescaped top-level `;` appearing
    // anywhere, not just inside `smtp.mailfrom=`) is still safe
    // REGARDLESS of provenance, via the SAME two defenses already in
    // place for a different delivery mechanism — the truncation fix
    // (if the injected `;` is immediately followed by another
    // stop-char) and, independently, the cross-section disagreement
    // check (if it instead forms a COMPLETE forged `dmarc=pass`
    // section ahead of the real, later, disagreeing one — exercised
    // here, since a fully-formed fake section is the stronger of the
    // two attacks).
    let header = "mx.google.com; \
                       dkim=pass header.i=attacker; dmarc=pass header.from=victim.io; \
                       dmarc=fail header.from=attacker.example";
    assert_eq!(
        dmarc_verdict(header),
        None,
        "an unescaped top-level ';' from ANY source (decoded property content included) \
             must never let a forged section win over a later, genuine, disagreeing one"
    );
}

// ── the exact exploit this module exists to close ──────────────────

#[test]
fn envelope_injected_text_inside_a_comment_and_smtp_mailfrom_does_not_override_a_genuine_fail() {
    // The reproduction from the fix-forward report: a SINGLE, GENUINE,
    // correctly-folded header — no forged second header, no
    // non-stamping host. RFC 5321 permits a QUOTED local-part in an
    // envelope MAIL FROM; the attacker picks
    // `"dmarc=pass header.from=greenhouse.io "` as their OWN envelope
    // local part, and Gmail's authentic SPF evaluation echoes it
    // verbatim — once inside a `(...)` comment, once as
    // `smtp.mailfrom=`'s own quoted-then-`@domain` pvalue — in the SPF
    // section, ahead of the REAL `dmarc=fail ...
    // header.from=attacker.example` section. A tokeniser that tracks
    // comment/quoted-string state must never treat either echo as a
    // `dmarc=` methodspec.
    let header = concat!(
        "mx.google.com; ",
        "dkim=pass header.i=@attacker.example header.s=selector header.b=xyz789; ",
        "spf=pass (google.com: domain of \"dmarc=pass header.from=greenhouse.io \"@attacker.example designates 5.6.7.8 as permitted sender) smtp.mailfrom=\"dmarc=pass header.from=greenhouse.io \"@attacker.example; ",
        "dmarc=fail (p=REJECT sp=REJECT dis=NONE) header.from=attacker.example"
    );
    assert_eq!(
        dmarc_verdict(header),
        Some(("fail".to_string(), "attacker.example".to_string())),
        "the REAL dmarc=fail section must win — the injected comment/quoted-string text must \
             never be read as a methodspec"
    );
}

#[test]
fn dmarc_inside_a_comment_is_not_a_methodspec() {
    let header = "mx.google.com; spf=pass (nothing about dmarc=pass here matters) \
                       smtp.mailfrom=bounce@attacker.example; \
                       dmarc=fail header.from=attacker.example";
    assert_eq!(
        dmarc_verdict(header),
        Some(("fail".to_string(), "attacker.example".to_string()))
    );
}

#[test]
fn dmarc_inside_a_quoted_string_is_not_a_methodspec() {
    let header = "mx.google.com; dkim=pass header.i=\"dmarc=pass header.from=greenhouse.io\"; \
                       dmarc=fail header.from=attacker.example";
    assert_eq!(
        dmarc_verdict(header),
        Some(("fail".to_string(), "attacker.example".to_string()))
    );
}

#[test]
fn dmarc_as_a_substring_of_another_property_name_is_not_a_methodspec() {
    // e.g. a hypothetical `x-dmarc-note=pass` property must not be
    // mistaken for the real `dmarc=` method — narrow-token reading
    // stops at whitespace/structural chars, so `x-dmarc-note` reads as
    // ONE token (a bare `name=value`), never split to expose `dmarc=`
    // as if it were its own methodspec.
    let header = "mx.google.com; x-dmarc-note=pass; dmarc=fail header.from=attacker.example";
    assert_eq!(
        dmarc_verdict(header),
        Some(("fail".to_string(), "attacker.example".to_string()))
    );
}

#[test]
fn header_from_in_a_different_section_is_not_attributed_to_dmarc() {
    // `header.from=` sitting in the DKIM section (a real, valid
    // propspec there too — DKIM ATPS uses it) must never be picked up
    // for a `dmarc` section that has none of its own.
    let header = "mx.google.com; \
                       dkim=pass header.i=@greenhouse.io header.from=greenhouse.io; \
                       dmarc=fail";
    assert_eq!(
        dmarc_verdict(header),
        None,
        "the dmarc section has no header.from of its OWN — must not borrow one from dkim's"
    );
}

#[test]
fn a_bare_from_property_without_the_header_ptype_prefix_is_not_header_from() {
    // A `from=` propspec with no `header.` ptype in front is a
    // DIFFERENT property entirely (bare `name=value`, ptype `None`) —
    // must never be mistaken for `header.from=` even though the
    // property NAME matches.
    let header = "mx.google.com; dmarc=pass from=greenhouse.io";
    assert_eq!(
        dmarc_verdict(header),
        None,
        "a bare `from=` (no `header.` ptype) must not satisfy header.from"
    );
}

// ── comment/quoted-string edge cases ────────────────────────────────

#[test]
fn an_unterminated_comment_fails_closed_not_panics() {
    let header =
        "mx.google.com; spf=pass (this comment never closes ; dmarc=pass header.from=greenhouse.io";
    assert_eq!(dmarc_verdict(header), None);
}

#[test]
fn an_unterminated_quoted_string_fails_closed_not_panics() {
    let header =
        "mx.google.com; dkim=pass header.i=\"never closes; dmarc=pass header.from=greenhouse.io";
    assert_eq!(dmarc_verdict(header), None);
}

// ── CRITICAL panic class: never slice an original string with an ───
// ── offset computed against a transformed one — this module never ──
// ── computes such an offset at all, but confirm the hostile input ──
// ── that used to reproduce the crash still just fails closed. ──────

#[test]
fn does_not_panic_on_the_originally_reported_char_boundary_crash() {
    let hostile = "mx.example.com; İ dmarc=épass header.from=greenhouse.io";
    let _ = dmarc_verdict(hostile);
}

#[test]
fn other_byte_length_changing_unicode_does_not_panic() {
    for hostile in [
        "mx.example.com; ß dmarc=pass header.from=greenhouse.io",
        "mx.example.com; K dmarc=pass header.from=greenhouse.io",
        "mx.example.com; Ꭰ dmarc=pass header.from=greenhouse.io",
    ] {
        let _ = dmarc_verdict(hostile);
    }
}

// ── structure ────────────────────────────────────────────────────────

#[test]
fn no_dmarc_section_at_all_is_none() {
    assert_eq!(
        dmarc_verdict("mx.google.com; dkim=pass; spf=pass smtp.mailfrom=bounce@greenhouse.io"),
        None
    );
}

#[test]
fn the_no_result_form_is_none_not_a_panic() {
    assert_eq!(dmarc_verdict("mx.google.com; none"), None);
}

#[test]
fn empty_and_whitespace_only_input_is_none() {
    assert_eq!(dmarc_verdict(""), None);
    assert_eq!(dmarc_verdict("   "), None);
}

#[test]
fn a_dmarc_section_with_no_header_from_is_none() {
    assert_eq!(dmarc_verdict("mx.google.com; dmarc=pass"), None);
}

#[test]
fn two_disagreeing_dmarc_sections_fail_closed() {
    let header = "mx.google.com; \
                       dmarc=pass header.from=greenhouse.io; \
                       dmarc=fail header.from=attacker.example";
    assert_eq!(
        dmarc_verdict(header),
        None,
        "two dmarc sections disagreeing must never silently pick a winner"
    );
}

// LOW fix: an empty method-version token (`dmarc/=pass`, or a comment
// standing in for the version like `dmarc/(c)=pass`) must fail closed
// rather than silently parse as "no version".
#[test]
fn an_empty_method_version_token_fails_the_section_closed() {
    let header = "mx.google.com; dmarc/=pass header.from=greenhouse.io";
    assert_eq!(
        dmarc_verdict(header),
        None,
        "a version slot with nothing in it is malformed, not absent"
    );
}

#[test]
fn a_comment_in_place_of_the_method_version_fails_the_section_closed() {
    let header = "mx.google.com; dmarc/(c)=pass header.from=greenhouse.io";
    assert_eq!(
        dmarc_verdict(header),
        None,
        "a comment is not a version token"
    );
}

// LOW, documented not fixed (see `Scanner::read_first_token`'s own
// doc): no authserv-id AND a method-version on the first section is
// fail-closed, not a false pass — pinned here so a future change to
// this area has to notice the behaviour, not just the doc.
#[test]
fn no_authserv_id_plus_a_method_version_on_the_first_section_fails_closed_not_open() {
    let header = "dmarc/1=pass header.from=greenhouse.io";
    assert_eq!(
        dmarc_verdict(header),
        None,
        "documents the known gap — a legitimate pass goes unrecognised, \
             never a forged one accepted"
    );
}
