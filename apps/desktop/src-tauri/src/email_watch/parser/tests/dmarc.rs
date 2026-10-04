use super::*;

// ── DMARC authentication (HIGH-2 fix) ────────────────────────────────────

#[test]
fn dmarc_pass_aligned_true_for_a_realistic_pass_header() {
    let ar = "mx.google.com; \
                   dkim=pass header.i=@greenhouse.io header.s=selector header.b=abc123; \
                   spf=pass smtp.mailfrom=bounce@greenhouse.io; \
                   dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=greenhouse.io"
        .to_string();
    assert!(dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
}

#[test]
fn dmarc_pass_aligned_false_when_result_is_not_pass() {
    let ar = "mx.google.com; dmarc=fail (p=REJECT) header.from=greenhouse.io".to_string();
    assert!(!dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
}

#[test]
fn dmarc_pass_aligned_false_when_header_from_does_not_match_the_visible_from_domain() {
    // A `pass` for a DIFFERENT domain than the visible `From:` does not
    // authenticate THIS sender — e.g. an attacker who controls DKIM for
    // some other domain but is spoofing the visible From: address of
    // a write-gate-eligible one.
    let ar = "mx.google.com; dmarc=pass (p=REJECT) header.from=attacker.example".to_string();
    assert!(!dmarc_pass_aligned(&[ar], Some("greenhouse.io")));
}

/// MINOR fix (doc-only, behavior unchanged and deliberately NOT
/// loosened): a genuine `header.from=greenhouse.io` stamp on a message
/// whose visible `From:` domain is the SUBDOMAIN
/// `mail.greenhouse.io` is legitimate DMARC relaxed alignment (RFC
/// 7489 organizational-domain match) but this fn's `eq_ignore_ascii_case`
/// is EXACT, so a real pass goes unrecognised — see this fn's own doc
/// for why that direction (under-matching, never over-trusting) is
/// safe and deliberate.
#[test]
fn dmarc_pass_aligned_is_stricter_than_dmarc_organizational_alignment() {
    let ar = "mx.google.com; dmarc=pass (p=REJECT) header.from=greenhouse.io".to_string();
    assert!(!dmarc_pass_aligned(&[ar], Some("mail.greenhouse.io")));
}

#[test]
fn dmarc_pass_aligned_fails_closed_on_a_missing_or_unparseable_header() {
    assert!(!dmarc_pass_aligned(&[], Some("greenhouse.io")));
    assert!(!dmarc_pass_aligned(
        &["not a valid authentication-results header at all".to_string()],
        Some("greenhouse.io")
    ));
    assert!(!dmarc_pass_aligned(
        &["mx.google.com; dmarc=pass (p=REJECT) header.from=greenhouse.io".to_string()],
        None
    ));
}

// -- LIVE, ACCEPTED RESIDUAL (not closed -- do not "fix" this test by --
// -- flipping its assertion; see parser.rs's own doc on dmarc_pass_aligned
// -- for the full reasoning, and auto_write.rs's doc for the mitigations)

#[test]
fn known_residual_a_genuine_stamp_with_no_dmarc_clause_of_its_own_can_still_be_forged() {
    // The gap `host_is_known_to_stamp` does NOT close, found by the
    // re-gate after the envelope-injection/truncation fixes: a GENUINE
    // stamp from a known-stamping host that simply carries no `dmarc=`
    // clause AT ALL for the message (a real, unremarkable outcome --
    // DMARC evaluation can legitimately produce a header with no dmarc
    // section for a given sender). The attacker picks their
    // `From:`/envelope domain specifically so the real stamp has this
    // shape, then supplies the header's ONLY `dmarc=` text themselves.
    //
    // What this test does NOT claim: a specific real-world delivery
    // mechanism. The comment/quoted-string escape this branch's OWN
    // truncation fix already closes (an unescaped `)` inside a quoted
    // local part) does NOT reach this outcome -- it was tried while
    // building this reproduction, and it leaves an orphaned quote
    // character that the truncation fix's "only genuine end-of-input
    // may stop the parse" rule correctly turns into `None`. What DOES
    // reach it, verified here, is an unquoted, unescaped `;` inside a
    // property value that RFC 8601 permits to be an addr-spec
    // (`smtp.mailfrom=`/`smtp.rcptto=`) rather than a strictly-quoted
    // token -- e.g. a verifying server that echoes an envelope
    // local-part into that property without re-quoting a character
    // its OWN grammar treats as structural. Whether any real,
    // deployed provider's stamping code has that specific gap is
    // exactly as unverifiable as the DKIM `i=`-decode question
    // recorded elsewhere in this file: this crate never constructs or
    // decodes that content itself, only reads whatever text a real
    // server already wrote. The STRUCTURAL point survives regardless
    // of which concrete mechanism a real attacker would reach for: an
    // unquoted top-level `;` with no genuine dmarc section to disagree
    // with cannot be told apart from real grammar, because it IS real
    // grammar by the time `dmarc_verdict` reads it -- there is nothing
    // malformed for a parser to reject.
    //
    // This assertion is `true` -- the vulnerable, unfortunate, but
    // CORRECT-for-what-this-function-can-know outcome -- on purpose,
    // matching how the (since-fixed) envelope-injection defect was
    // originally recorded: prove the exploit is real rather than
    // assert it away. Unlike that one, THIS residual is not expected
    // to close via a parser change (two candidate fixes were measured
    // and both failed) -- what changed instead is the write path's OWN
    // default: `EmailWatchStore::auto_write_enabled` now defaults OFF,
    // and every write this whole pipeline can ever produce still lands
    // UNCONFIRMED, requiring the user's own adjudication. Those are
    // the real mitigations; this function's `true` here is not one of
    // them.
    let header = "mx.google.com; \
                       dkim=pass header.i=attacker;dmarc=pass header.from=victim.io"
        .to_string();
    assert!(
        dmarc_pass_aligned(&[header], Some("victim.io")),
        "documents the live residual — see this test's own doc before treating a change \
             here as a fix"
    );
}

// -- HIGH fix: only the TOPMOST Authentication-Results is trustworthy ----
//
// RFC 8601 SS5: a consumer must only trust the header field ADDED BY ITS
// OWN receiving MTA and must ignore any that arrived already present in
// the message -- the final MTA PREPENDS its own stamp, so in document
// order the FIRST occurrence is the one the user's own provider just
// added; everything below it is either an earlier hop or attacker text
// and must never be consulted. `.any()` over every occurrence let an
// attacker satisfy the gate just by including their own forged header
// naming a write-gate domain -- these are the permanent regression
// tests for that exploit; this class returns the moment someone
// "simplifies" the header scan back to a search.
//
// NOT tested here, and NOT closeable by this function (see its own doc
// for the full reasoning): a message where the ONLY
// `Authentication-Results` header present is a forged one that already
// matches the exact string a genuine stamp would contain (e.g. an
// attacker who correctly writes `mx.google.com` -- the coordinator's
// own worked example). No text-only parse of a single header can tell
// that apart from a real one; only cryptographic re-verification (DKIM/
// SPF, performed independently by this code against DNS) closes it, and
// that is a substantial new feature, not built here.

#[test]
fn dmarc_pass_aligned_uses_only_the_topmost_stamp_ignoring_a_forged_one_below() {
    // A genuine topmost pass, aligned to the write-gate domain, PLUS a
    // forged second header underneath claiming a pass for a completely
    // different domain. Must authorize based on the topmost ONLY -- the
    // forged second header must never be consulted, let alone able to
    // redirect which domain gets authorized.
    let genuine_topmost =
        "mx.google.com; dmarc=pass (p=REJECT) header.from=greenhouse.io".to_string();
    let forged_below = "attacker.example; dmarc=pass header.from=attacker.example".to_string();
    assert!(dmarc_pass_aligned(
        &[genuine_topmost.clone(), forged_below.clone()],
        Some("greenhouse.io")
    ));
    // And the forged domain must NOT be authorized either, even though
    // it is present in the list -- confirms the lower header is truly
    // ignored, not merely "the wrong one happened to lose."
    assert!(!dmarc_pass_aligned(
        &[genuine_topmost, forged_below],
        Some("attacker.example")
    ));
}

#[test]
fn dmarc_pass_aligned_a_genuine_topmost_fail_is_not_overridden_by_a_forged_pass_below() {
    // THE CASE `.any()` GETS EXACTLY BACKWARDS: the AUTHORITATIVE
    // (topmost, genuine) result says `dmarc=fail`. A forged header
    // below it claims `pass` for the same domain. Must NOT authorize --
    // `.any()` would find the forged `pass` and return true regardless
    // of what the real evaluation said.
    let genuine_fail = "mx.google.com; dmarc=fail (p=REJECT) header.from=greenhouse.io".to_string();
    let forged_pass = "attacker.example; dmarc=pass header.from=greenhouse.io".to_string();
    assert!(
        !dmarc_pass_aligned(&[genuine_fail, forged_pass], Some("greenhouse.io")),
        "a genuine topmost fail must never be overridden by a forged pass beneath it"
    );
}

#[test]
fn dmarc_pass_aligned_fails_closed_when_the_topmost_header_is_unreadable() {
    // The topmost header parses to `None` (malformed/unexpected shape)
    // -- must fail closed, never fall through to a LOWER header (which
    // would reintroduce the same trust-order violation as `.any()`).
    let unreadable = "not a valid authentication-results header at all".to_string();
    let genuine_below_it =
        "mx.google.com; dmarc=pass (p=REJECT) header.from=greenhouse.io".to_string();
    assert!(!dmarc_pass_aligned(
        &[unreadable, genuine_below_it],
        Some("greenhouse.io")
    ));
}

// -- CRITICAL fix: this used to be a byte-index desync between a --------
// -- lowercased offset and an original-case slice; the new tokeniser ---
// -- (`super::auth_results`) has no offset arithmetic to desync at all, -
// -- but re-run the exact reproductions here too, end-to-end through ---
// -- the actual public entry point every caller uses. -------------------

#[test]
fn dmarc_pass_aligned_does_not_panic_on_hostile_unicode_and_fails_closed() {
    // 'İ' (U+0130) is THE ORIGINAL reported reproduction: it
    // lowercases to a byte-length-changing sequence ("i̇", 2 bytes ->
    // 3), which is exactly what desynced the OLD substring scanner's
    // offsets (`panic = "abort"` in release turned that into the whole
    // desktop app crashing, permanently, since the crashing message
    // gets re-fetched every launch). ß/K (Kelvin sign)/Ꭰ (Cherokee)
    // are the other byte-length/codepoint-count-changing cases already
    // pinned. None should panic; none should authorize.
    for hostile in [
        "mx.example.com; İ dmarc=épass header.from=greenhouse.io",
        "mx.example.com; ß dmarc=pass header.from=greenhouse.io",
        "mx.example.com; K dmarc=pass header.from=greenhouse.io",
        "mx.example.com; Ꭰ dmarc=pass header.from=greenhouse.io",
    ] {
        assert!(!dmarc_pass_aligned(
            &[hostile.to_string()],
            Some("greenhouse.io")
        ));
    }
}

// -- Recall corpus: does the scanner read REAL providers' shapes correctly? --
//
// Every fixture on this branch before this corpus was Gmail-shaped, so
// the false-negative rate on other major providers was never measured.
// PROVENANCE, stated plainly rather than silently assumed: this
// environment has no live network/web access, so these are NOT
// captured from a real inbox. They are reconstructed from each
// provider's OWN publicly documented `Authentication-Results` header
// format (Microsoft's own "Anti-spam message headers in Microsoft 365"
// support article for the M365 shape; the general RFC 8601 `resinfo`
// conventions each of Yahoo/Fastmail/Proton is documented to follow
// elsewhere) plus this crate author's general training knowledge of
// real header samples -- NOT verified against a current live message.
// Treat this as a reasonable-effort starting corpus, not ground truth;
// exact authserv-id strings, comment wording, and folding details can
// drift from a real captured header, and only real captured headers
// from each provider (from someone with live inbox access) can close
// that gap. Pin BOTH directions per provider: a genuine pass for a
// write-gate domain must authorise; a genuine fail (or hostile input)
// must not -- a gate that silently refuses every legitimate email is
// also a broken feature.

/// Does the scanner read REAL providers' `Authentication-Results` shapes correctly? Pins BOTH
/// directions per provider: a genuine pass for a write-gate domain authorises, a genuine fail does
/// not. The label is the former test's name.
#[test]
fn recall_corpus_genuine_pass_authorises_and_genuine_fail_does_not() {
    let cases: &[(&str, &str, bool)] = &[
        // Microsoft's documented shape has NO leading `<authserv-id>;` --
        // it starts directly with `spf=`, and carries Microsoft-specific
        // extension properties (`action=`, `compauth=`) RFC 8601 does not
        // define. Confirms clause-selection does not require an
        // authserv-id prefix to work.
        (
            "recall_outlook_m365_genuine_pass_authorises: a genuine M365 pass must authorise -- Microsoft's authserv-id-less shape must not \
             be silently refused",
            "spf=pass (sender IP is 40.107.1.1) smtp.mailfrom=greenhouse.io; \
                   dkim=pass (signature was verified) header.d=greenhouse.io header.s=selector1; \
                   dmarc=pass action=none header.from=greenhouse.io; \
                   compauth=pass reason=100",
            true,
        ),
        (
            "recall_outlook_m365_genuine_fail_does_not_authorise",
            "spf=softfail (sender IP is 203.0.113.9) smtp.mailfrom=attacker.example; \
                   dkim=none header.d=none; \
                   dmarc=fail action=none header.from=greenhouse.io; \
                   compauth=fail reason=001",
            false,
        ),
        // Yahoo is documented to sometimes omit the space before the
        // parenthesized comment (`dmarc=pass(p=REJECT)` not
        // `dmarc=pass (p=REJECT)`).
        (
            "recall_yahoo_genuine_pass_authorises",
            "mtaX.mail.gq1.yahoo.com; \
                   dkim=pass (ok) header.i=@greenhouse.io header.s=s2048 header.b=abcdefgh; \
                   spf=pass smtp.mailfrom=bounce@greenhouse.io; \
                   dmarc=pass(p=REJECT) header.from=greenhouse.io",
            true,
        ),
        (
            "recall_yahoo_genuine_fail_does_not_authorise",
            "mtaX.mail.gq1.yahoo.com; \
                   dkim=fail header.i=@attacker.example header.s=s2048 header.b=abcdefgh; \
                   spf=fail smtp.mailfrom=bounce@attacker.example; \
                   dmarc=fail(p=REJECT) header.from=greenhouse.io",
            false,
        ),
        (
            "recall_fastmail_genuine_pass_authorises",
            "mx-fm-int.internal; \
                   dkim=pass (2048-bit rsa key sha256) header.d=greenhouse.io header.i=@greenhouse.io header.b=abcdef; \
                   dmarc=pass (p=NONE sp=NONE dis=NONE) header.from=greenhouse.io; \
                   spf=pass smtp.mailfrom=bounce@greenhouse.io",
            true,
        ),
        (
            "recall_fastmail_genuine_fail_does_not_authorise",
            "mx-fm-int.internal; \
                   dkim=none; \
                   dmarc=fail (p=NONE sp=NONE dis=NONE) header.from=greenhouse.io; \
                   spf=none smtp.mailfrom=bounce@attacker.example",
            false,
        ),
        // Reached only via ProtonMail Bridge (a LOCAL IMAP proxy on
        // 127.0.0.1) -- this test is about the CONTENT scanner only; see
        // `host_is_known_to_stamp`'s own doc for why the account-level
        // gate deliberately does NOT extend `host_is_known_to_stamp`
        // coverage to Proton (the bridge's loopback address carries no
        // signal about the real provider behind it).
        (
            "recall_protonmail_genuine_pass_authorises",
            "mail.protonmail.ch; \
                   dkim=pass (2048-bit key) header.d=greenhouse.io header.b=abcdef; \
                   dmarc=pass (p=reject sp=reject) header.from=greenhouse.io; \
                   spf=pass smtp.mailfrom=bounce@greenhouse.io",
            true,
        ),
        (
            "recall_protonmail_genuine_fail_does_not_authorise",
            "mail.protonmail.ch; \
                   dkim=fail header.d=attacker.example header.b=abcdef; \
                   dmarc=fail (p=reject sp=reject) header.from=greenhouse.io; \
                   spf=fail smtp.mailfrom=bounce@attacker.example",
            false,
        ),
    ];
    for (label, header, authorised) in cases {
        assert_eq!(
            dmarc_pass_aligned(&[header.to_string()], Some("greenhouse.io")),
            *authorised,
            "{label}"
        );
    }
}

// -- FIXED: the envelope-injection exploit that survived two prior ------
// -- rounds of substring-scanning. This test used to assert the -------
// -- VULNERABLE outcome (`true`), deliberately, so CI could never claim -
// -- it was resolved. It is flipped here because the underlying scanner
// -- was actually replaced (`super::auth_results`, an RFC 8601 --------
// -- tokeniser that tracks comment/quoted-string state) -- not because -
// -- the assertion was edited in isolation. --------------------------

#[test]
fn envelope_injected_text_no_longer_overrides_a_genuine_fail_verdict() {
    // The exploit: a SINGLE, GENUINE, correctly-folded header -- no
    // forged second header, no non-stamping host. RFC 5321 permits a
    // QUOTED local-part in an envelope MAIL FROM address (`"any text
    // including = and spaces"@domain`); Gmail's SPF evaluation is
    // genuinely authentic (attacker.example is the attacker's OWN
    // domain, so its SPF record can genuinely authorise it), and
    // Gmail's real `Authentication-Results` header echoes that
    // attacker-CHOSEN local-part verbatim inside the SPF section's
    // comment/`smtp.mailfrom=` property. The attacker picks the local
    // part to literally BE `dmarc=pass header.from=greenhouse.io `,
    // injecting exactly the text a SUBSTRING scanner's clause-selection
    // was looking for -- INTO an earlier (SPF) section, ahead of the
    // REAL `dmarc=fail ... header.from=attacker.example` section that
    // comes later in the SAME genuine header.
    //
    // A tokeniser that tracks comment/quoted-string state (see
    // `super::auth_results::dmarc_verdict`) never surfaces either echo
    // as a `dmarc=` methodspec -- the injected text lives inside a
    // DIFFERENT section's OWN comment/quoted pvalue, structurally
    // distinct from a real `dmarc=` token, not merely a sharper
    // heuristic about WHICH occurrence to trust.
    let ar = "mx.google.com; \
                   dkim=pass header.i=@attacker.example header.s=selector header.b=xyz789; \
                   spf=pass (google.com: domain of \"dmarc=pass header.from=greenhouse.io \"@attacker.example designates 5.6.7.8 as permitted sender) smtp.mailfrom=\"dmarc=pass header.from=greenhouse.io \"@attacker.example; \
                   dmarc=fail (p=REJECT sp=REJECT dis=NONE) header.from=attacker.example"
            .to_string();
    assert!(
        !dmarc_pass_aligned(&[ar], Some("greenhouse.io")),
        "the REAL dmarc=fail section must win -- the injected comment/quoted-string text \
             must never be read as a methodspec"
    );
}

// ── parse_header: the Authentication-Results header wired through to `dmarc_pass` ──

#[test]
fn parse_header_wires_a_real_authentication_results_header_through_to_dmarc_pass() {
    // End-to-end: the raw fetched bytes -> mail-parser -> dmarc_pass_aligned
    // path, not just the pure helper tested above in isolation.
    // Deliberately unfolded (one physical line for the whole header
    // value) — a `\`-continued Rust byte-string literal strips leading
    // whitespace from the next line, so a folded/indented continuation
    // written the "natural" way here would silently NOT reproduce RFC
    // 5322 folding and would parse as a malformed header instead.
    let raw = b"From: Careers <careers@greenhouse.io>\r\n\
Subject: Thank you for applying!\r\n\
Authentication-Results: mx.google.com; dkim=pass header.i=@greenhouse.io header.s=selector header.b=abc; dmarc=pass (p=REJECT) header.from=greenhouse.io\r\n\
\r\n";
    let header = parse_header(raw).expect("should parse a minimal header block");
    assert_eq!(header.from_domain.as_deref(), Some("greenhouse.io"));
    assert!(
        header.dmarc_pass,
        "a real, aligned dmarc=pass header must wire through"
    );
}

#[test]
fn parse_header_end_to_end_proves_mail_parser_preserves_document_order_for_repeated_headers() {
    // The whole topmost-only fix rests on `mail_parser` collecting
    // repeated headers in DOCUMENT order (never reversed, never
    // deduped) -- verified here against a REAL raw message through the
    // REAL parser, not just by reading the crate's source (which was
    // also checked: `MessageStream::parse_headers` in
    // mail-parser-0.11.6/src/parsers/header.rs scans the byte stream
    // forward and `Vec::push`es each header as it's encountered, and
    // `Message::header_as` in src/core/message.rs iterates that Vec in
    // order and collects matches -- but a live assertion is the
    // authoritative check, not the source read). TWO
    // `Authentication-Results` headers: a genuine topmost pass for
    // greenhouse.io, a forged second one below claiming a pass for
    // attacker.example. If mail-parser ever reversed or reordered
    // repeated headers, this test would start authorizing the WRONG
    // domain and fail loudly, rather than the vulnerability silently
    // reappearing. Built via `.join("\r\n")` rather than a
    // `\`-continued byte-string literal -- that continuation style
    // already bit one earlier test in this file by silently eating
    // the leading whitespace of a folded line.
    let raw = [
        "From: Careers <careers@greenhouse.io>",
        "Subject: Thank you for applying!",
        "Authentication-Results: mx.google.com; dmarc=pass (p=REJECT) header.from=greenhouse.io",
        "Authentication-Results: attacker.example; dmarc=pass header.from=attacker.example",
        "",
        "",
    ]
    .join("\r\n");
    let header = parse_header(raw.as_bytes()).expect("should parse a minimal header block");
    assert!(
        header.dmarc_pass,
        "the genuine topmost header (greenhouse.io, matching the visible From:) must authorize"
    );
}

#[test]
fn parse_header_dmarc_pass_is_false_with_no_authentication_results_header_at_all() {
    // The absent-header half of "fail closed" — a host that never
    // stamps this header (a plain forward, a non-Gmail-shaped IMAP
    // provider) must never accidentally read as authenticated.
    let raw = b"From: Careers <careers@greenhouse.io>\r\n\
Subject: Thank you for applying!\r\n\
\r\n";
    let header = parse_header(raw).expect("should parse a minimal header block");
    assert!(!header.dmarc_pass);
}
