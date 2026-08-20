//! A small, total, single-pass RFC 8601 tokeniser over ONE
//! `Authentication-Results` header value — replaces three rounds of
//! substring-scanning, the last of which a genuine, correctly-folded,
//! single Gmail header could defeat: an attacker-chosen envelope-from
//! local part, echoed verbatim by Gmail's own authentic SPF evaluation
//! inside that SAME header's SPF section, contained the literal text
//! `dmarc=pass header.from=<domain>`. A substring scan cannot tell a
//! comment's or quoted-string's CONTENT from a real `dmarc=` methodspec,
//! because it never tracked what kind of syntax it was walking through.
//! This module does: it tracks comment depth (RFC 5322 `(...)` comments
//! NEST) and quoted-string state as it walks, so text inside either is
//! NEVER mistaken for a token — a structural fix, not a sharper heuristic.
//!
//! Deliberately narrow — exactly what finding ONE `dmarc` verdict needs,
//! not a general MIME/RFC 5322 parser: `[authserv-id [version]] ; resinfo
//! ...`, where a `resinfo` is `methodspec [reasonspec] *propspec`.
//! `authserv-id` is technically REQUIRED by strict RFC 8601 grammar but is
//! treated as OPTIONAL here — Microsoft 365's real header shape omits it
//! and goes straight into the first section's `methodspec` (see
//! `microsoft_shaped_header_with_no_leading_authserv_id_parses`). Total
//! (never panics) and fails closed (`None`) on anything not confidently
//! read — malformed input is exactly as safe as absent input.
//!
//! No slicing an offset found in one string against a different one (the
//! CRITICAL byte-boundary panic class fixed earlier this branch, in the
//! substring scanner this module replaces) — every token here is built by
//! pushing `char`s read directly off THIS scanner into an owned `String`,
//! so there is no byte-offset arithmetic left to desync at all.

/// Find the `dmarc` resinfo section in `header` (one raw
/// `Authentication-Results` value) and return its `(result, header.from)`
/// pair — e.g. `("pass", "greenhouse.io")`, taken verbatim (the caller
/// compares case-insensitively). `None` on anything not confidently read:
/// no `dmarc` section at all, malformed structure, an unterminated
/// comment or quoted string, a `dmarc` section with no `header.from`
/// property, or more than one `dmarc` section whose result/`header.from`
/// disagree — never silently pick a winner between two disagreeing claims
/// in the same header.
pub(super) fn dmarc_verdict(header: &str) -> Option<(String, String)> {
    let mut sc = Scanner::new(header);
    let mut found: Option<(String, String)> = None;

    sc.skip_cfws().ok()?;

    // `authserv-id` is REQUIRED by strict grammar, but Microsoft 365's
    // real shape omits it and goes straight into the first section's
    // `methodspec`. Read one token; if it is immediately followed by '='
    // (never valid directly after a conformant authserv-id — that is
    // always followed by CFWS, a version, or ';'), it WAS actually the
    // first section's method name, not an authserv-id — process it as
    // such via `pending_method` rather than discarding it.
    let first_token = if sc.peek() == Some('"') {
        sc.read_quoted_string().ok()?
    } else {
        sc.read_first_token()
    };
    sc.skip_cfws().ok()?;
    let mut pending_method = None;
    if sc.peek() == Some('=') {
        pending_method = Some(first_token);
    } else if sc.peek().is_some_and(|c| c.is_ascii_digit()) {
        // Optional authres-version (`1*DIGIT`) — vanishingly rare in
        // practice, but cheap to tolerate rather than fail on.
        sc.read_narrow_token();
        sc.skip_cfws().ok()?;
    }
    // else: `first_token` genuinely was the authserv-id, with nothing else
    // before the first ';' (or the header ends here) — nothing more to do.

    loop {
        let method = if let Some(m) = pending_method.take() {
            m
        } else {
            sc.skip_cfws().ok()?;
            match sc.peek() {
                Some(';') => {
                    sc.next();
                }
                None => break,
                Some(_) => return None, // expected ';' (next resinfo) or end
            }
            sc.skip_cfws().ok()?;
            let method = sc.read_narrow_token();
            if method.is_empty() {
                // Either the `no-result` form's bare "none" (no methodspec
                // follows a no-result ";"), or malformed. Either way
                // nothing after it is safely interpretable as a fresh
                // resinfo without risking an infinite loop — stop.
                break;
            }
            method
        };

        match process_section(&mut sc, method).ok()? {
            SectionOutcome::NotDmarc | SectionOutcome::DmarcNoHeaderFrom => {}
            SectionOutcome::Dmarc(result, header_from) => match &found {
                None => found = Some((result, header_from)),
                Some((prev_result, prev_from)) => {
                    if !result.eq_ignore_ascii_case(prev_result)
                        || !header_from.eq_ignore_ascii_case(prev_from)
                    {
                        // Two `dmarc` sections in one header, disagreeing
                        // — never silently pick one.
                        return None;
                    }
                }
            },
        }
    }

    found
}

enum SectionOutcome {
    NotDmarc,
    DmarcNoHeaderFrom,
    Dmarc(String, String),
}

/// Process ONE resinfo section's `[CFWS] "=" [CFWS] result` (the scanner is
/// positioned right after `method`, i.e. at the optional `"/"
/// method-version` or the `"="`) followed by zero or more properties, up
/// to the next `;` or end of input. Extracted from [`dmarc_verdict`] so it
/// can be called EITHER from the normal `; method=...` loop, or directly
/// with a `method` already read (the no-authserv-id case).
fn process_section(sc: &mut Scanner, method: String) -> Result<SectionOutcome, ()> {
    sc.skip_cfws()?;
    if sc.peek() == Some('/') {
        sc.next();
        sc.skip_cfws()?;
        sc.read_narrow_token(); // method-version, discarded
        sc.skip_cfws()?;
    }
    if sc.peek() != Some('=') {
        return Err(());
    }
    sc.next();
    sc.skip_cfws()?;
    let result = sc.read_value()?;

    let is_dmarc = method.eq_ignore_ascii_case("dmarc");
    let mut section_header_from: Option<String> = None;

    // Zero or more properties: a dotted RFC 8601 propspec
    // (`ptype.property=pvalue`) or a bare `name=value` pair (the
    // `reasonspec`, or any other non-dotted `name=value` a real server
    // emits, e.g. Microsoft's `action=`/`compauth=`) — unified here since
    // both just need to be consumed correctly to keep the scanner
    // positioned right; only the DOTTED `header.from=` pair, and only
    // while `is_dmarc`, is ever captured.
    loop {
        sc.skip_cfws()?;
        if matches!(sc.peek(), Some(';') | None) {
            break;
        }
        let first = sc.read_narrow_token();
        if first.is_empty() {
            return Err(()); // no forward progress possible — malformed
        }
        sc.skip_cfws()?;
        let (ptype, property) = if sc.peek() == Some('.') {
            sc.next();
            sc.skip_cfws()?;
            let property = sc.read_narrow_token();
            if property.is_empty() {
                return Err(());
            }
            (Some(first), property)
        } else {
            (None, first)
        };
        sc.skip_cfws()?;
        if sc.peek() != Some('=') {
            return Err(());
        }
        sc.next();
        sc.skip_cfws()?;
        let pvalue = sc.read_value()?;

        if is_dmarc
            && ptype
                .as_deref()
                .is_some_and(|p| p.eq_ignore_ascii_case("header"))
            && property.eq_ignore_ascii_case("from")
        {
            section_header_from = Some(pvalue);
        }
    }

    Ok(if !is_dmarc {
        SectionOutcome::NotDmarc
    } else if let Some(header_from) = section_header_from {
        SectionOutcome::Dmarc(result, header_from)
    } else {
        SectionOutcome::DmarcNoHeaderFrom
    })
}

/// A tokeniser over one header value's characters, tracking exactly the
/// two states RFC 5322 CFWS/quoted-string parsing needs: is the current
/// position inside a (possibly nested) comment, and is it inside a quoted
/// string. Neither state's CONTENT is ever surfaced as a token — that is
/// the entire fix. Built on `Peekable<Chars>`: every token is assembled by
/// pushing characters read directly off this iterator into an owned
/// `String`, never by slicing `src` with a computed byte offset.
struct Scanner<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Scanner<'a> {
    fn new(src: &'a str) -> Self {
        Scanner {
            chars: src.chars().peekable(),
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn next(&mut self) -> Option<char> {
        self.chars.next()
    }

    /// Skip RFC 5322 CFWS: folding whitespace and any number of
    /// (possibly nested) comments. `Err(())` on an unterminated comment.
    fn skip_cfws(&mut self) -> Result<(), ()> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.next();
                }
                Some('(') => self.skip_comment()?,
                _ => return Ok(()),
            }
        }
    }

    /// Skip one balanced, possibly-nested `(...)` comment — RFC 5322
    /// comments nest, and a backslash quoted-pair escapes the NEXT
    /// character (including a literal `(`/`)`) without affecting nesting
    /// depth. A `"` inside a comment has NO special meaning (unlike inside
    /// a quoted-string) — RFC 5322 `ccontent` never nests a quoted-string,
    /// so it is never treated as one here either. Assumes
    /// `peek() == Some('(')`; `Err(())` if it never closes.
    fn skip_comment(&mut self) -> Result<(), ()> {
        debug_assert_eq!(self.peek(), Some('('));
        self.next();
        let mut depth: u32 = 1;
        while depth > 0 {
            match self.next() {
                Some('\\') => {
                    if self.next().is_none() {
                        return Err(()); // trailing backslash, no escaped char
                    }
                }
                Some('(') => depth += 1,
                Some(')') => depth -= 1,
                Some(_) => {}
                None => return Err(()),
            }
        }
        Ok(())
    }

    /// Read a `"..."` quoted string (assumes `peek() == Some('"')`),
    /// honouring backslash escapes, and return its UNESCAPED content.
    /// `Err(())` if it never closes.
    fn read_quoted_string(&mut self) -> Result<String, ()> {
        debug_assert_eq!(self.peek(), Some('"'));
        self.next();
        let mut out = String::new();
        loop {
            match self.next() {
                Some('"') => return Ok(out),
                Some('\\') => match self.next() {
                    Some(c) => out.push(c),
                    None => return Err(()),
                },
                Some(c) => out.push(c),
                None => return Err(()),
            }
        }
    }

    /// An unquoted token stopping at whitespace or any structural
    /// delimiter this grammar uses (`;` `.` `=` `(` `)` `"` `/`) — used
    /// for `method`/`ptype`/`property` NAMES, which are RFC 8601 `Keyword`
    /// (letters, digits, hyphens only) and so never legitimately contain
    /// any of those anyway.
    fn read_narrow_token(&mut self) -> String {
        self.read_token_until(|c| {
            c.is_whitespace() || matches!(c, ';' | '.' | '=' | '(' | ')' | '"' | '/')
        })
    }

    /// An unquoted token stopping ONLY at whitespace or `;` `(` `)` `"` —
    /// deliberately permissive about `.`, `=`, `/`, `@`: valid content
    /// inside a domain name, an email address, or a non-quoted base64
    /// fragment (`header.b=`) — used for `pvalue`/`result`/`authserv-id`
    /// segments (see [`Self::read_value`], which also handles `=`
    /// correctly since IT decides section boundaries, not this fn).
    fn read_wide_token(&mut self) -> String {
        self.read_token_until(|c| c.is_whitespace() || matches!(c, ';' | '(' | ')' | '"'))
    }

    /// Like [`Self::read_wide_token`], but ALSO stops at `=` — used ONLY
    /// for the very FIRST token of the header, to disambiguate a
    /// conventional `authserv-id` (never legitimately followed directly by
    /// `=`) from a `methodspec`'s `method` name, which Microsoft 365's
    /// no-authserv-id shape puts there instead (see [`dmarc_verdict`]'s
    /// own doc). Not used anywhere else: an authserv-id containing a
    /// literal, unquoted `=` is not valid per grammar either way, so
    /// stopping there is safe for BOTH interpretations.
    fn read_first_token(&mut self) -> String {
        self.read_token_until(|c| c.is_whitespace() || matches!(c, ';' | '(' | ')' | '"' | '='))
    }

    fn read_token_until(&mut self, stop: impl Fn(char) -> bool) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if stop(c) {
                break;
            }
            out.push(c);
            self.next();
        }
        out
    }

    /// RFC 8601 `value`/`pvalue`, content NEEDED — handles the ordinary
    /// `token` / `quoted-string` forms, AND the `smtp.mailfrom=`/
    /// `smtp.rcptto=` addr-spec shape (`[local-part] "@" domain-name`)
    /// where a QUOTED local-part is followed IMMEDIATELY (no CFWS) by more
    /// unquoted content (`@` and the domain) — this module's own
    /// regression test for the envelope-injection exploit exercises
    /// EXACTLY this shape. Reads and concatenates quoted/unquoted segments
    /// for as long as they continue with NO intervening whitespace, so the
    /// scanner's POSITION ends up correct either way — this fn's caller
    /// never inspects a captured `pvalue`'s content structurally (only
    /// compares the whole string against an expected domain), so exactly
    /// how a mixed quoted+unquoted value gets concatenated does not
    /// matter, only that parsing does not desync afterward.
    fn read_value(&mut self) -> Result<String, ()> {
        let mut out = String::new();
        loop {
            if self.peek() == Some('"') {
                out.push_str(&self.read_quoted_string()?);
            } else {
                let tok = self.read_wide_token();
                if tok.is_empty() {
                    break; // nothing left this fn can read as content
                }
                out.push_str(&tok);
            }
            match self.peek() {
                Some(c) if c.is_whitespace() || matches!(c, ';' | '(' | ')') => break,
                None => break,
                _ => {} // more content glued on with no CFWS — keep reading
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── the exact exploit this module exists to close ──────────────────

    #[test]
    fn envelope_injected_text_inside_a_comment_and_smtp_mailfrom_does_not_override_a_genuine_fail()
    {
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

    #[test]
    fn an_escaped_close_paren_inside_a_comment_does_not_end_it_early() {
        // `\)` inside a comment is a quoted-pair (escapes the character,
        // does not affect nesting depth) — must not be read as the REAL
        // closing paren, which would leave the scanner desynced for
        // everything after.
        let header =
            "mx.google.com; spf=pass (a \\) literal paren) smtp.mailfrom=bounce@greenhouse.io; \
                       dmarc=pass header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    #[test]
    fn multibyte_content_inside_a_quoted_pvalue_is_read_correctly() {
        // Confirms the char-based (never byte-offset) design handles
        // arbitrary multi-byte UTF-8 inside a value without corrupting
        // the surrounding structure — an emoji and an accented character,
        // both multi-byte in UTF-8, embedded in an otherwise-irrelevant
        // quoted property value ahead of the real dmarc section.
        let header = "mx.google.com; dkim=pass header.i=\"café 🎉 note\"; \
                       dmarc=pass header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    // ── comment/quoted-string edge cases ────────────────────────────────

    #[test]
    fn nested_comments_are_skipped_as_one_unit() {
        let header = "mx.google.com; spf=pass (outer (inner (deepest) still inner) still outer) \
                       smtp.mailfrom=bounce@greenhouse.io; \
                       dmarc=pass header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    #[test]
    fn an_unterminated_comment_fails_closed_not_panics() {
        let header = "mx.google.com; spf=pass (this comment never closes ; dmarc=pass header.from=greenhouse.io";
        assert_eq!(dmarc_verdict(header), None);
    }

    #[test]
    fn an_unterminated_quoted_string_fails_closed_not_panics() {
        let header = "mx.google.com; dkim=pass header.i=\"never closes; dmarc=pass header.from=greenhouse.io";
        assert_eq!(dmarc_verdict(header), None);
    }

    #[test]
    fn an_escaped_quote_inside_a_quoted_string_does_not_end_it_early() {
        let header = "mx.google.com; dkim=pass header.i=\"a \\\" quote\"; \
                       dmarc=pass header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string())),
            "an escaped quote inside the DKIM quoted-string must not be read as its closing quote"
        );
    }

    #[test]
    fn a_quoted_header_from_pvalue_is_read_correctly() {
        let header = "mx.google.com; dmarc=pass header.from=\"greenhouse.io\"";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    #[test]
    fn a_quoted_local_part_glued_to_an_unquoted_domain_is_consumed_as_one_pvalue() {
        // The addr-spec shape (`smtp.mailfrom="local part"@domain`) that
        // the envelope-injection exploit relies on — verified directly
        // here (not just via the end-to-end exploit test above): the
        // scanner must land correctly on the FOLLOWING `;`, not get
        // confused by the unquoted `@domain` glued onto the quoted part.
        let header =
            "mx.google.com; spf=pass smtp.mailfrom=\"quoted local part\"@attacker.example; \
                       dmarc=pass header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
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
    fn two_agreeing_dmarc_sections_authorise() {
        let header = "mx.google.com; \
                       dmarc=pass header.from=greenhouse.io; \
                       dmarc=pass header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
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

    #[test]
    fn method_version_is_tolerated() {
        let header = "mx.google.com; dmarc/1=pass header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    #[test]
    fn microsoft_shaped_header_with_no_leading_authserv_id_parses() {
        let header = "spf=pass (sender IP is 40.107.1.1) smtp.mailfrom=greenhouse.io; \
                       dkim=pass (signature was verified) header.d=greenhouse.io; \
                       dmarc=pass action=none header.from=greenhouse.io; \
                       compauth=pass reason=100";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string())),
            "a leading authserv-id must not be REQUIRED to find the dmarc section"
        );
    }

    #[test]
    fn a_realistic_gmail_pass_header_parses() {
        let header = "mx.google.com; \
                       dkim=pass header.i=@greenhouse.io header.s=selector header.b=abc123; \
                       spf=pass (google.com: domain of bounce@greenhouse.io designates 1.2.3.4 as permitted sender) smtp.mailfrom=bounce@greenhouse.io; \
                       dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }
}
