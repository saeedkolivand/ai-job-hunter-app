//! `decode_plaintext`/`decrypt_value`/`decrypt_v10_aes_gcm` tests: the
//! hash-prefix-strip heuristic, v10/v11 AES-GCM round-trips, and the
//! undecryptable-outcome edge cases.

use super::super::decrypt::{decode_plaintext, decrypt_v10_aes_gcm, decrypt_value, DecryptResult};

/// Encrypt `plaintext` with AES-256-GCM and assemble a Chromium-style
/// `<prefix><nonce><ciphertext+tag>` blob — the shared fixture every v10/v11
/// round-trip test below builds and then decrypts.
fn make_encrypted_blob(
    prefix: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
    plaintext: &[u8],
) -> Vec<u8> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let cipher = Aes256Gcm::new_from_slice(key).unwrap();
    let ct = cipher
        .encrypt(<&Nonce<_>>::try_from(&nonce[..]).unwrap(), plaintext)
        .unwrap();
    let mut blob = prefix.to_vec();
    blob.extend_from_slice(nonce);
    blob.extend_from_slice(&ct);
    blob
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

// ── Gap 1: AES-GCM v10 round-trip ────────────────────────────────────────────

/// Build a well-formed Chromium v10 blob: `b"v10"` + 12-byte nonce + AES-256-GCM
/// ciphertext+tag, then verify `decrypt_v10_aes_gcm` returns the exact original
/// plaintext bytes. This exercises the entire nonce-slice / ct-slice layout
/// documented in the module (nonce = enc[3..15], ct+tag = enc[15..]).
#[test]
fn decrypt_v10_aes_gcm_round_trip_exact_bytes() {
    let key = [0x42u8; 32]; // arbitrary deterministic 32-byte key
    let nonce = [0x11u8; 12]; // arbitrary deterministic 12-byte nonce
    let plaintext = b"li_at_session_token_value_abc123";
    let blob = make_encrypted_blob(b"v10", &key, &nonce, plaintext);

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
    let key = [0xABu8; 32];
    let nonce = [0x55u8; 12];
    let plaintext = b"AQEDAT-linked-in-token";
    let blob = make_encrypted_blob(b"v10", &key, &nonce, plaintext);

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
    let encrypt_key = [0x01u8; 32];
    let wrong_key = [0x02u8; 32];
    let nonce = [0x00u8; 12];
    let blob = make_encrypted_blob(b"v10", &encrypt_key, &nonce, b"secret");

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
    let key = [0x77u8; 32];
    let nonce = [0x33u8; 12];
    let real_value = b"glassdoor_sess_token_xyz";

    // Simulate the Chromium hash prefix: 32 bytes all \x00 (non-printable).
    let mut prefixed = vec![0u8; 32];
    prefixed.extend_from_slice(real_value);
    let blob = make_encrypted_blob(b"v10", &key, &nonce, &prefixed);

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
    let key = [0xCCu8; 32];
    let nonce = [0x99u8; 12];
    let plaintext = b"xing_session_token_value_v11";
    // v11 layout is identical to v10.
    let blob = make_encrypted_blob(b"v11", &key, &nonce, plaintext);

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
