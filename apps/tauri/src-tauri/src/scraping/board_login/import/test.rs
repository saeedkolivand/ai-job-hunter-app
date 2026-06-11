//! Unit tests for the pure, platform-independent cookie-import helpers.

use super::*;

#[test]
fn domain_matches_linkedin() {
    assert!(domain_matches("linkedin", ".linkedin.com"));
    assert!(domain_matches("linkedin", "linkedin.com"));
    assert!(domain_matches("linkedin", "www.linkedin.com"));
    assert!(!domain_matches("linkedin", "indeed.com"));
    // `.`-anchored suffix must reject lookalike hosts that merely start with
    // the board domain.
    assert!(!domain_matches("linkedin", "linkedin.com.evil.test"));
    assert!(!domain_matches("linkedin", "notlinkedin.com"));
}

#[test]
fn domain_matches_indeed_locale_tlds() {
    for host in [
        "indeed.com",
        ".de.indeed.com",
        "de.indeed.com",
        "uk.indeed.com",
        "indeed.fr",
        "ca.indeed.com",
    ] {
        assert!(domain_matches("indeed", host), "should match {host}");
    }
    assert!(!domain_matches("indeed", "linkedin.com"));
    // Must not match an unrelated host that merely contains the letters.
    assert!(!domain_matches("indeed", "notindeedreally.example"));
    assert!(domain_matches("indeed", "notindeed.indeed.com"));
}

#[test]
fn domain_matches_xing_glassdoor() {
    assert!(domain_matches("xing", "www.xing.com"));
    assert!(domain_matches("xing", "xing.com"));
    assert!(domain_matches("glassdoor", ".glassdoor.com"));
    assert!(domain_matches("glassdoor", "glassdoor.com"));
    assert!(!domain_matches("xing", "glassdoor.com"));
    // `.`-anchored suffix rejects lookalike hosts for these boards too.
    assert!(!domain_matches("xing", "xing.com.evil.test"));
    assert!(!domain_matches("glassdoor", "glassdoor.com.evil.test"));
}

#[test]
fn domain_matches_unknown_board_is_false() {
    assert!(!domain_matches("monster", "monster.com"));
}

#[test]
fn chromium_time_session_cookie_is_none() {
    assert_eq!(chromium_time_to_unix(0), None);
    assert_eq!(chromium_time_to_unix(-1), None);
}

#[test]
fn chromium_time_converts_known_epoch() {
    // 1601 epoch micros for exactly unix epoch (1970-01-01) = EPOCH_DELTA_US.
    const EPOCH_DELTA_US: i64 = 11_644_473_600_000_000;
    assert_eq!(chromium_time_to_unix(EPOCH_DELTA_US), None); // delta -> 0 -> None
                                                             // One second after unix epoch.
    let v = chromium_time_to_unix(EPOCH_DELTA_US + 1_000_000).unwrap();
    assert!((v - 1.0).abs() < 1e-6, "got {v}");
}

#[test]
fn decode_plaintext_plain_ascii() {
    match decode_plaintext(b"AQEDAT-token".to_vec()) {
        DecryptResult::Plain(s) => assert_eq!(s, "AQEDAT-token"),
        DecryptResult::Undecryptable => panic!("expected plain"),
    }
}

#[test]
fn decode_plaintext_strips_binary_hash_prefix() {
    // 32 non-printable bytes followed by an ASCII value -> hash stripped.
    let mut bytes = vec![0u8; 32];
    bytes.extend_from_slice(b"li_at_value_123");
    match decode_plaintext(bytes) {
        DecryptResult::Plain(s) => assert_eq!(s, "li_at_value_123"),
        DecryptResult::Undecryptable => panic!("expected stripped plain"),
    }
}

#[test]
fn decode_plaintext_empty_is_plain_empty() {
    match decode_plaintext(Vec::new()) {
        DecryptResult::Plain(s) => assert!(s.is_empty()),
        DecryptResult::Undecryptable => panic!("expected empty plain"),
    }
}

#[test]
fn decrypt_value_v20_is_undecryptable() {
    let mut blob = b"v20".to_vec();
    blob.extend_from_slice(&[0u8; 40]);
    assert!(matches!(
        decrypt_value(&blob, Some(&[0u8; 32])),
        DecryptResult::Undecryptable
    ));
}

#[test]
fn decrypt_value_v10_without_key_is_undecryptable() {
    let mut blob = b"v10".to_vec();
    blob.extend_from_slice(&[0u8; 40]);
    assert!(matches!(
        decrypt_value(&blob, None),
        DecryptResult::Undecryptable
    ));
}

#[test]
fn outcome_serializes_pascal_case() {
    assert_eq!(
        serde_json::to_value(ImportOutcome::BrowserNotFound).unwrap(),
        serde_json::json!("BrowserNotFound")
    );
    assert_eq!(
        serde_json::to_value(ImportOutcome::NoSession).unwrap(),
        serde_json::json!("NoSession")
    );
    // Tuple variant serializes as { "Imported": n } — the command layer maps it
    // to a flat shape, but PascalCase naming is what we assert here.
    assert_eq!(
        serde_json::to_value(ImportOutcome::Imported(3)).unwrap(),
        serde_json::json!({ "Imported": 3 })
    );
}

#[test]
fn import_unknown_board_is_no_session() {
    let tmp = std::env::temp_dir();
    let outcome = import_cookies(&tmp, "definitely-not-a-board").unwrap();
    assert_eq!(outcome, ImportOutcome::NoSession);
}

#[test]
fn path_with_suffix_appends() {
    let p = std::path::Path::new("/x/Cookies");
    assert_eq!(
        path_with_suffix(p, "-wal"),
        std::path::PathBuf::from("/x/Cookies-wal")
    );
}

// ── Gap 1: AES-GCM v10 round-trip ────────────────────────────────────────────

/// Build a well-formed Chromium v10 blob: `b"v10"` + 12-byte nonce + AES-256-GCM
/// ciphertext+tag, then verify `decrypt_v10_aes_gcm` returns the exact original
/// plaintext bytes. This exercises the entire nonce-slice / ct-slice layout
/// documented in the module (nonce = enc[3..15], ct+tag = enc[15..]).
#[test]
fn decrypt_v10_aes_gcm_round_trip_exact_bytes() {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let key = [0x42u8; 32]; // arbitrary deterministic 32-byte key
    let nonce_bytes = [0x11u8; 12]; // arbitrary deterministic 12-byte nonce
    let plaintext = b"li_at_session_token_value_abc123";

    // Encrypt with the same algorithm the production code decrypts.
    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct_with_tag = cipher.encrypt(nonce, plaintext.as_ref()).unwrap();

    // Assemble the Chromium-style blob.
    let mut blob = b"v10".to_vec();
    blob.extend_from_slice(&nonce_bytes); // bytes[3..15]
    blob.extend_from_slice(&ct_with_tag); // bytes[15..]

    // Call the internal AES-GCM decryptor directly (reachable via `use super::*`).
    let got = decrypt_v10_aes_gcm(&blob, &key).expect("AES-GCM decrypt must succeed");
    assert_eq!(
        got, plaintext,
        "decrypted bytes must exactly match plaintext"
    );
}

/// v11 prefix is treated identically to v10 by the dispatch in `decrypt_value`;
/// ensure the round-trip works end-to-end through `decrypt_value` as well.
#[test]
fn decrypt_value_v10_full_pipeline_returns_plaintext() {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let key = [0xABu8; 32];
    let nonce_bytes = [0x55u8; 12];
    let plaintext = b"AQEDAT-linked-in-token";

    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    let ct_with_tag = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_ref())
        .unwrap();

    let mut blob = b"v10".to_vec();
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ct_with_tag);

    match decrypt_value(&blob, Some(&key)) {
        DecryptResult::Plain(s) => {
            assert_eq!(s.as_bytes(), plaintext, "value must round-trip exactly");
        }
        DecryptResult::Undecryptable => panic!("expected Plain, got Undecryptable"),
    }
}

/// Wrong key must yield `Undecryptable` (GCM tag verification fails).
#[test]
fn decrypt_v10_aes_gcm_wrong_key_is_none() {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let encrypt_key = [0x01u8; 32];
    let wrong_key = [0x02u8; 32];
    let nonce_bytes = [0x00u8; 12];

    let cipher = Aes256Gcm::new_from_slice(&encrypt_key).unwrap();
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), b"secret".as_ref())
        .unwrap();

    let mut blob = b"v10".to_vec();
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ct);

    // `decrypt_v10_aes_gcm` returns None on auth failure.
    assert!(
        decrypt_v10_aes_gcm(&blob, &wrong_key).is_none(),
        "wrong key must not decrypt"
    );
}

/// Blob too short to contain nonce+tag (< 3+12+16 = 31 bytes) must return None.
#[test]
fn decrypt_v10_aes_gcm_too_short_returns_none() {
    let key = [0u8; 32];
    // 30 bytes total — one byte under the minimum.
    let blob: Vec<u8> = b"v10".iter().chain(&[0u8; 27]).copied().collect();
    assert_eq!(blob.len(), 30);
    assert!(decrypt_v10_aes_gcm(&blob, &key).is_none());
}

/// AES-GCM round-trip where the plaintext is prefixed with 32 non-printable
/// bytes (the SHA-256 domain hash some Chromium builds prepend). The full
/// pipeline via `decrypt_value` must strip the prefix and return the clean value.
#[test]
fn decrypt_value_v10_strips_32_byte_hash_prefix() {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let key = [0x77u8; 32];
    let nonce_bytes = [0x33u8; 12];
    let real_value = b"glassdoor_sess_token_xyz";

    // Simulate the Chromium hash prefix: 32 bytes all \x00 (non-printable).
    let mut prefixed = vec![0u8; 32];
    prefixed.extend_from_slice(real_value);

    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), prefixed.as_ref())
        .unwrap();

    let mut blob = b"v10".to_vec();
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ct);

    match decrypt_value(&blob, Some(&key)) {
        DecryptResult::Plain(s) => {
            assert_eq!(
                s.as_bytes(),
                real_value,
                "32-byte hash prefix must be stripped"
            );
        }
        DecryptResult::Undecryptable => panic!("expected Plain after prefix strip"),
    }
}

// ── Gap 2: decode_plaintext edge cases ───────────────────────────────────────

/// A 32-byte prefix that is ALL printable ASCII must NOT be stripped — the
/// heuristic only fires when the first 32 bytes contain non-printable bytes.
#[test]
fn decode_plaintext_printable_32_byte_prefix_not_stripped() {
    // 32 printable ASCII bytes + a recognisable suffix.
    let mut bytes = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_vec(); // exactly 32 'A'
    assert_eq!(bytes.len(), 32);
    bytes.extend_from_slice(b"-suffix");

    // The whole thing is valid UTF-8 and the first 32 bytes are graphic ASCII,
    // so the value must come through intact, not stripped.
    match decode_plaintext(bytes) {
        DecryptResult::Plain(s) => {
            // Exact round-trip: the 32 'A's + "-suffix" must come through intact.
            // A partial strip (e.g. stripping the 32-byte prefix) would produce
            // "-suffix" (7 chars), so a precise equality check is the only assertion
            // that would catch such a regression.
            assert_eq!(
                s, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA-suffix",
                "full 39-char string must be preserved when prefix is printable"
            );
        }
        DecryptResult::Undecryptable => {
            panic!("printable 32-byte prefix must not become Undecryptable")
        }
    }
}

/// Fewer than 32 bytes that are NOT valid UTF-8 → `Undecryptable`.
/// The strip path requires `bytes.len() > 32`, so a short non-UTF-8 buffer
/// must not panic and must return `Undecryptable`.
#[test]
fn decode_plaintext_short_invalid_utf8_is_undecryptable() {
    // 4 bytes of invalid UTF-8 — below the 32-byte strip threshold.
    let bytes = vec![0xFF, 0xFE, 0x80, 0x81];
    assert!(matches!(
        decode_plaintext(bytes),
        DecryptResult::Undecryptable
    ));
}

/// Non-printable prefix of exactly 32 bytes where the STRIPPED tail is also
/// non-UTF-8 → `Undecryptable` (no silent data corruption).
#[test]
fn decode_plaintext_non_utf8_tail_after_strip_is_undecryptable() {
    // 32 non-printable bytes (trigger strip path) + 4 invalid UTF-8 bytes.
    let mut bytes = vec![0x01u8; 32];
    bytes.extend_from_slice(&[0xFF, 0xFE, 0x80, 0x81]);
    // Neither the full buffer nor the 32-stripped tail is valid UTF-8.
    assert!(matches!(
        decode_plaintext(bytes),
        DecryptResult::Undecryptable
    ));
}

// ── Gap 3: domain_matches anchoring edge cases ────────────────────────────────

/// indeed locale TLDs: direct second-level domains like `indeed.de` and
/// `indeed.co.uk` must match (the `contains("indeed.")` rule fires on the
/// `.` after `indeed`).
#[test]
fn domain_matches_indeed_direct_locale_tlds() {
    for host in ["indeed.de", "indeed.co.uk", "indeed.in", "indeed.com.au"] {
        assert!(
            domain_matches("indeed", host),
            "must match indeed locale TLD: {host}"
        );
    }
}

/// The indeed rule uses `host.contains("indeed.")` (the dot is the key guard).
/// A host that does NOT contain the literal string `"indeed."` must not match.
/// NOTE: hosts like `someindeed.com` *do* contain `"indeed."` (at char offset 4)
/// and therefore DO match — this is a known, documented heuristic limitation;
/// we assert the actual behaviour here rather than an idealised one.
#[test]
fn domain_matches_indeed_rejects_substring_without_label() {
    // "indeedish.io" → "indeedish.io".contains("indeed.") == false → must not match.
    assert!(!domain_matches("indeed", "indeedish.io"));
    // "noindeedhere.net" → does not contain "indeed." → must not match.
    assert!(!domain_matches("indeed", "noindeedhere.net"));
    // Document the known heuristic: "someindeed.com" contains "indeed." so it
    // DOES match (false-positive, accepted trade-off for locale-TLD coverage).
    assert!(
        domain_matches("indeed", "someindeed.com"),
        "known heuristic: someindeed.com contains 'indeed.' and matches — document, not fix"
    );
}

/// The `.`-anchored suffix check for linkedin must accept an uppercase input
/// (the code normalises via `to_ascii_lowercase`).
#[test]
fn domain_matches_linkedin_case_insensitive() {
    assert!(domain_matches("linkedin", "WWW.LINKEDIN.COM"));
    assert!(domain_matches("linkedin", "LinkedIn.Com"));
    // Lookalike with uppercase must still be rejected.
    assert!(!domain_matches("linkedin", "LINKEDIN.COM.EVIL.TEST"));
}

// ── Gap 4: ImportOutcome serde completeness ───────────────────────────────────

/// `Undecryptable` was missing from the existing serde test. Assert the exact
/// JSON string the frontend contract expects.
#[test]
fn outcome_undecryptable_serializes_pascal_case() {
    assert_eq!(
        serde_json::to_value(ImportOutcome::Undecryptable).unwrap(),
        serde_json::json!("Undecryptable")
    );
}

/// All four variants together — one table-driven assertion to catch any future
/// rename that breaks the IPC contract.
#[test]
fn outcome_all_variants_ipc_contract() {
    use serde_json::json;

    let cases: &[(ImportOutcome, serde_json::Value)] = &[
        (ImportOutcome::Imported(0), json!({ "Imported": 0 })),
        (ImportOutcome::Imported(42), json!({ "Imported": 42 })),
        (ImportOutcome::NoSession, json!("NoSession")),
        (ImportOutcome::Undecryptable, json!("Undecryptable")),
        (ImportOutcome::BrowserNotFound, json!("BrowserNotFound")),
    ];

    for (variant, expected) in cases {
        let got = serde_json::to_value(variant).unwrap();
        assert_eq!(got, *expected, "serde mismatch for variant");
    }
}

// ── HIGH 1: is_authed_cookies predicate ──────────────────────────────────────

fn make_cookie(name: &str, value: &str, domain: &str) -> super::super::StoredCookie {
    super::super::StoredCookie {
        name: name.to_owned(),
        value: value.to_owned(),
        domain: domain.to_owned(),
        path: "/".to_owned(),
        expires: None,
        http_only: false,
        secure: true,
    }
}

/// linkedin: a valid `li_at` (value.len() > 10, domain contains "linkedin.com") → true.
#[test]
fn is_authed_cookies_linkedin_valid_li_at_returns_true() {
    let predicate = crate::scraping::board_login::get_config("linkedin")
        .unwrap()
        .is_authed_cookies
        .unwrap();

    let cookies = vec![make_cookie("li_at", "a_valid_token_1234", ".linkedin.com")];
    assert!(
        predicate(&cookies),
        "valid li_at with linkedin.com domain must return true"
    );
}

/// linkedin: cookies present but NO `li_at` → false.
#[test]
fn is_authed_cookies_linkedin_no_li_at_returns_false() {
    let predicate = crate::scraping::board_login::get_config("linkedin")
        .unwrap()
        .is_authed_cookies
        .unwrap();

    let cookies = vec![
        make_cookie("JSESSIONID", "some_session_value_xyz", ".linkedin.com"),
        make_cookie("bscookie", "another_cookie_value_abc", ".linkedin.com"),
    ];
    assert!(
        !predicate(&cookies),
        "absent li_at must return false even with other linkedin cookies"
    );
}

/// linkedin: `li_at` present but value too short (len <= 10) → false.
#[test]
fn is_authed_cookies_linkedin_li_at_too_short_returns_false() {
    let predicate = crate::scraping::board_login::get_config("linkedin")
        .unwrap()
        .is_authed_cookies
        .unwrap();

    // Exactly 10 chars — the condition is `value.len() > 10`, so this must fail.
    let cookies = vec![make_cookie("li_at", "0123456789", ".linkedin.com")];
    assert!(
        !predicate(&cookies),
        "li_at value.len() == 10 must return false (condition is > 10, not >= 10)"
    );

    // 5 chars — also too short.
    let short = vec![make_cookie("li_at", "short", ".linkedin.com")];
    assert!(
        !predicate(&short),
        "li_at value.len() < 10 must return false"
    );
}

/// linkedin: `li_at` present but wrong domain → false.
#[test]
fn is_authed_cookies_linkedin_wrong_domain_returns_false() {
    let predicate = crate::scraping::board_login::get_config("linkedin")
        .unwrap()
        .is_authed_cookies
        .unwrap();

    // Value is long enough but domain does not contain "linkedin.com".
    let cookies = vec![make_cookie(
        "li_at",
        "a_valid_token_1234",
        ".evil-linkedin.io",
    )];
    assert!(
        !predicate(&cookies),
        "li_at with wrong domain must return false"
    );
}

/// indeed / xing / glassdoor: `is_authed_cookies` must be `None` — documents the
/// "any non-empty cookie set → Imported" None-arm policy in `import_cookies`.
#[test]
fn is_authed_cookies_is_none_for_indeed_xing_glassdoor() {
    for board_id in ["indeed", "xing", "glassdoor"] {
        let cfg = crate::scraping::board_login::get_config(board_id)
            .unwrap_or_else(|| panic!("get_config({board_id}) must return Some"));
        assert!(
            cfg.is_authed_cookies.is_none(),
            "is_authed_cookies for {board_id} must be None (any non-empty cookie set → Imported)"
        );
    }
}

// ── HIGH 2: decrypt_value empty-blob path ────────────────────────────────────

/// An empty blob must return `Plain("")` — NOT `Undecryptable`. The early-return
/// guard `if enc.is_empty()` must fire before any version-prefix branch.
#[test]
fn decrypt_value_empty_blob_returns_plain_empty() {
    let key = [0u8; 32];
    match decrypt_value(&[], Some(&key)) {
        DecryptResult::Plain(s) => assert!(
            s.is_empty(),
            "empty blob must produce Plain(\"\"), got Plain({s:?})"
        ),
        DecryptResult::Undecryptable => {
            panic!("empty blob must return Plain(\"\"), not Undecryptable")
        }
    }
}

// ── MEDIUM 4: v11 round-trip ─────────────────────────────────────────────────

/// `v11` prefix is dispatched identically to `v10` in `decrypt_value`. Verify
/// the full end-to-end pipeline produces the original plaintext.
#[test]
fn decrypt_value_v11_round_trip_returns_plaintext() {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let key = [0xCCu8; 32];
    let nonce_bytes = [0x99u8; 12];
    let plaintext = b"xing_session_token_value_v11";

    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    let ct_with_tag = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_ref())
        .unwrap();

    // Assemble a v11-prefixed blob (identical layout to v10).
    let mut blob = b"v11".to_vec();
    blob.extend_from_slice(&nonce_bytes); // bytes[3..15]
    blob.extend_from_slice(&ct_with_tag); // bytes[15..]

    match decrypt_value(&blob, Some(&key)) {
        DecryptResult::Plain(s) => {
            assert_eq!(
                s.as_bytes(),
                plaintext,
                "v11 blob must round-trip through decrypt_value exactly"
            );
        }
        DecryptResult::Undecryptable => panic!("v11 blob must decrypt; got Undecryptable"),
    }
}
