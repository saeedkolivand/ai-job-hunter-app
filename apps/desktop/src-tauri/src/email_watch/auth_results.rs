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
//! **That "fails closed" claim was FALSE for one release**, and it is the
//! kind of sentence a reviewer trusts instead of re-deriving, so the
//! defect it hid is worth naming here rather than only in a commit
//! message: the main loop used to `break` (returning whatever `dmarc`
//! verdict had ALREADY been found) whenever the next token read empty —
//! which happens not only at genuine end-of-input but also right after
//! ANY stray top-level `;`, `.`, `=`, `)`, `"`, or `/` immediately
//! following a `;`. A single genuine header with a real `dmarc=fail`
//! section could still be made to authorise: an attacker-controlled
//! envelope local part containing an unescaped `)` legitimately closes an
//! EARLIER section's comment early (RFC 5322 `ccontent` gives `"` no
//! special meaning inside a comment, so a quoted-string cannot hide a `)`
//! there the way it can hide one from itself), landing a stray `;;` right
//! after a FORGED `dmarc=pass` clause the attacker placed inside that same
//! comment — the loop then stopped BEFORE ever reaching the real,
//! genuine, later `dmarc=fail` section, and returned the forged one
//! instead. Fixed by requiring genuine end-of-input (`peek()` is `None`)
//! before an empty token may `break`; anything else with input remaining
//! `return`s `None`. In a fail-closed parser, `break` is the dangerous
//! keyword, not `unwrap` — every loop exit in this module is checked
//! against this same shape, not just the one that was wrong.
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
                // CRITICAL fix: an empty token here means the char right
                // after `skip_cfws` was one of `; . = ) " /` -- i.e.
                // `read_narrow_token` stopped on its VERY FIRST char. If
                // that char is real content (peek is `Some`, not `None`),
                // this is NOT a legitimate end of input -- it is a stray
                // top-level delimiter (`;;`, `; "`, `; =`, `; .`, `; /`,
                // `; )`), and falling through to `break` would silently
                // truncate the parse and return whatever `found` ALREADY
                // held -- a stale, possibly-genuine-looking verdict from
                // an EARLIER section, while the REAL section after the
                // stray delimiter (which could disagree, e.g. a genuine
                // `dmarc=fail`) is never read at all. Only a TRUE
                // end-of-input (`peek()` is `None`) may `break` — that is
                // the sole case where "nothing more to parse" is actually
                // true, matching the `no-result` form's bare "none" (which
                // itself reads as a non-empty token and never reaches this
                // branch) and a cleanly-terminated header.
                if sc.peek().is_some() {
                    return None;
                }
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
        // LOW fix: the version token was read but never checked for
        // emptiness, so `dmarc/=pass` (nothing between '/' and '=') or
        // `dmarc/(c)=pass` (a comment where a version should be) would
        // silently accept a missing version rather than reject malformed
        // input — not exploitable (no way to use this to smuggle a
        // competing verdict past the checks below), but a version slot
        // that accepts "no version" is not doing its job.
        if sc.read_narrow_token().is_empty() {
            return Err(());
        }
        sc.skip_cfws()?;
    }
    if sc.peek() != Some('=') {
        return Err(());
    }
    sc.next();
    sc.skip_cfws()?;
    let result = sc.read_value()?;
    // `result` CAN be `""` (`read_value`'s own loop-exit audit covers why
    // that's a safe stopping point, not a desync): the char right after
    // `=` is a stop char before any content is read, e.g. `dmarc=;`. Do
    // NOT assume that reaches the `result.eq_ignore_ascii_case("pass")`
    // check downstream and safely fails it — an EMPTY pvalue does reach
    // that shape (a blank `header.from=` value still gets captured into
    // `section_header_from` below and is compared against a real domain
    // later), but an empty RESULT never gets the chance: hitting `;`
    // immediately after `=` ALSO means the propspec loop below breaks on
    // its very first iteration, so `section_header_from` stays `None` and
    // this section returns `SectionOutcome::DmarcNoHeaderFrom` a few lines
    // down — silently dropped by `dmarc_verdict` before `result` is ever
    // compared to anything. The safety property still holds (an attacker
    // supplying `dmarc=;` cannot corrupt a genuine LATER section, because
    // this section never touches `found`), but by "vanishes unread", not
    // "read and safely fails an equality check" — those are different
    // mechanisms and only the former applies here.
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
            // MEDIUM fix: a SECOND `header.from=` within this ONE section
            // used to be last-wins, which the cross-section disagreement
            // rule below did not mirror — that asymmetry was not
            // exploitable today only because of `dmarc_pass_aligned`'s own
            // gate and how four surveyed providers happen to order their
            // properties, i.e. borrowed safety, not structural safety.
            // Fail closed on disagreement here too, exactly like two
            // disagreeing `dmarc` SECTIONS — never silently pick a winner
            // at any level.
            match &section_header_from {
                None => section_header_from = Some(pvalue),
                Some(existing) if existing.eq_ignore_ascii_case(&pvalue) => {}
                Some(_) => return Err(()),
            }
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
                // `saturating_add`: LOW fix -- `depth` is otherwise
                // unchecked `u32` arithmetic (out of model at realistic
                // header sizes -- would need ~4 GiB of nested `(` to
                // overflow -- but removing the last unchecked arithmetic
                // in this module costs nothing). Saturating rather than a
                // hard depth cap: an attacker forcing saturation just
                // means every subsequent `)` decrements one step closer to
                // 0 instead of truly balancing, which only makes the
                // comment MORE likely to (correctly) run off the end of
                // input and fail closed via `None => return Err(())`
                // below -- never a way to escape the comment early.
                Some('(') => depth = depth.saturating_add(1),
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
    ///
    /// KNOWN GAP, documented rather than fixed: this does not stop at
    /// `/`, so a header with BOTH no leading authserv-id AND a
    /// method-version on that first section (`dmarc/1=pass
    /// header.from=…`) reads `dmarc/1` as one token instead of `dmarc`
    /// then a version. `dmarc_verdict` then finds `peek() == Some('=')`
    /// (unchanged by this bug) and treats the whole `"dmarc/1"` string as
    /// the method name, which fails `method.eq_ignore_ascii_case("dmarc")`
    /// in `process_section` — the section is misclassified as
    /// `NotDmarc`, and with nothing else in the header `dmarc_verdict`
    /// returns `None`. Fail-closed, not a security gap: the worst outcome
    /// is a legitimate pass going unrecognised, never a forged one
    /// accepted. Not fixed here because doing so properly needs a real
    /// branch below for `peek() == Some('/')` mirroring
    /// [`process_section`]'s own version handling (including this
    /// branch's own emptiness check) — genuine new logic in the same
    /// fail-closed loop that took three review rounds to harden, not a
    /// two-line change, for a shape (no authserv-id AND a version on the
    /// very first section) no surveyed real provider produces.
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
    fn truncation_attack_stray_quote_after_semicolon() {
        let header = "mx.google.com; dmarc=pass header.from=victim.io; \" \
                       dmarc=fail header.from=attacker.example";
        assert_eq!(dmarc_verdict(header), None);
    }

    #[test]
    fn truncation_attack_stray_equals_after_semicolon() {
        let header = "mx.google.com; dmarc=pass header.from=victim.io; = \
                       dmarc=fail header.from=attacker.example";
        assert_eq!(dmarc_verdict(header), None);
    }

    #[test]
    fn truncation_attack_stray_dot_after_semicolon() {
        let header = "mx.google.com; dmarc=pass header.from=victim.io; . \
                       dmarc=fail header.from=attacker.example";
        assert_eq!(dmarc_verdict(header), None);
    }

    #[test]
    fn truncation_attack_stray_slash_after_semicolon() {
        let header = "mx.google.com; dmarc=pass header.from=victim.io; / \
                       dmarc=fail header.from=attacker.example";
        assert_eq!(dmarc_verdict(header), None);
    }

    #[test]
    fn truncation_attack_stray_close_paren_after_semicolon() {
        let header = "mx.google.com; dmarc=pass header.from=victim.io; ) \
                       dmarc=fail header.from=attacker.example";
        assert_eq!(dmarc_verdict(header), None);
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

    // ── legitimate shapes the truncation fix must NOT break ─────────────
    // ── (a gate that refuses every legitimate email is as broken as one──
    // ── that lets attackers through) ─────────────────────────────────────

    #[test]
    fn legitimate_trailing_semicolon_with_nothing_after_still_authorises() {
        let header = "mx.google.com; dmarc=pass header.from=greenhouse.io;";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    #[test]
    fn legitimate_trailing_semicolon_with_whitespace_still_authorises() {
        let header = "mx.google.com; dmarc=pass header.from=greenhouse.io; ";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    #[test]
    fn legitimate_trailing_semicolon_then_a_comment_still_authorises() {
        let header =
            "mx.google.com; dmarc=pass header.from=greenhouse.io; (trailing note, no more sections)";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    #[test]
    fn legitimate_yahoo_no_space_before_the_comment_still_authorises() {
        // Yahoo's documented shape: `dmarc=pass(p=REJECT)`, no space
        // between the result and the parenthesized comment.
        let header = "mtaX.mail.gq1.yahoo.com; dmarc=pass(p=REJECT) header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    #[test]
    fn legitimate_fastmail_shaped_tail_with_arc_and_policy_and_x_prefixed_properties() {
        // Fastmail-style tail: an `arc=none` section (no properties at
        // all), and a dmarc section carrying non-standard `policy.*`/
        // `x-*`-prefixed properties alongside the real `header.from=`.
        // None of these must interfere with finding the real verdict.
        let header = "in1-smtp.messagingengine.com; \
                       arc=none; \
                       dmarc=pass policy.published-domain=greenhouse.io policy.applied-disposition=none \
                       header.from=greenhouse.io x-spam-score=0.0";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    // ── MEDIUM fix: a duplicate header.from WITHIN one section must be ──
    // ── symmetric with the cross-section disagreement rule ──────────────

    #[test]
    fn duplicate_header_from_within_one_section_agreeing_is_fine() {
        let header =
            "mx.google.com; dmarc=pass header.from=greenhouse.io header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
    }

    #[test]
    fn duplicate_header_from_within_one_section_disagreeing_fails_closed() {
        let header =
            "mx.google.com; dmarc=pass header.from=greenhouse.io header.from=attacker.example";
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

    // LOW fix (comment correction, not behaviour): pins that an empty
    // RESULT (`dmarc=;`) vanishes unread rather than being captured and
    // failing an equality check — see the comment on `process_section`'s
    // `let result = sc.read_value()?;` line for the full mechanism. This
    // is documented, intended behaviour, not a bug: the empty-result
    // section never reaches `found`, so it cannot corrupt the genuine
    // section that follows it.
    #[test]
    fn an_empty_dmarc_result_vanishes_unread_and_does_not_block_a_later_genuine_pass() {
        let header = "mx.google.com; dmarc=; dmarc=pass header.from=greenhouse.io";
        assert_eq!(
            dmarc_verdict(header),
            Some(("pass".to_string(), "greenhouse.io".to_string()))
        );
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
