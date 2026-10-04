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
mod tests;
