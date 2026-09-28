use super::*;

// ── Cross-implementation known-answer vector ───────────────────────────────
// These EXACT values are also asserted by the TS side
// (`apps/extension/src/lib/handshake.test.ts` via Web Crypto, and
// `packages/shared/.../extension-protocol-constants.ts::HANDSHAKE_TEST_VECTOR`).
// If the Rust (hmac crate) and TS (Web Crypto) byte-canonicalizations ever
// drift, one side's KAT fails loudly instead of the handshake silently never
// matching. DO NOT edit one side without recomputing the other.
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const CLIENT_NONCE: &str = "00112233445566778899aabbccddeeff";
const SERVER_NONCE: &str = "ffeeddccbbaa99887766554433221100";
const CLIENT_PROOF: &str = "fe16f06234473b154c4e96d43bd25c603975cb2584f950d0d4f495edc5c44f1a";
const SERVER_PROOF: &str = "75c05269902c14d97ee61a05f4c9dbf812c532735b836665da250b19ce405831";

#[test]
fn handshake_message_is_canonical() {
    assert_eq!(
        handshake_message(ROLE_CLIENT, SERVER_NONCE, CLIENT_NONCE),
        "ajh-bridge/v2\nclient\nffeeddccbbaa99887766554433221100\n00112233445566778899aabbccddeeff"
    );
    assert_eq!(
        handshake_message(ROLE_SERVER, SERVER_NONCE, CLIENT_NONCE),
        "ajh-bridge/v2\nserver\nffeeddccbbaa99887766554433221100\n00112233445566778899aabbccddeeff"
    );
}

#[test]
fn kat_client_and_server_proofs_match_shared_vector() {
    assert_eq!(
        client_proof(TOKEN, SERVER_NONCE, CLIENT_NONCE),
        CLIENT_PROOF,
        "Rust client proof must equal the shared cross-impl vector"
    );
    assert_eq!(
        server_proof(TOKEN, SERVER_NONCE, CLIENT_NONCE),
        SERVER_PROOF,
        "Rust server proof must equal the shared cross-impl vector"
    );
}

#[test]
fn client_and_server_proofs_differ_by_role() {
    // Domain separation: swapping the role must change the proof, so a client
    // proof can never be replayed as a server proof (or vice versa).
    assert_ne!(
        client_proof(TOKEN, SERVER_NONCE, CLIENT_NONCE),
        server_proof(TOKEN, SERVER_NONCE, CLIENT_NONCE)
    );
}

#[test]
fn verify_accepts_the_correct_proof() {
    assert!(verify_client_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        CLIENT_PROOF
    ));
}

#[test]
fn verify_rejects_a_tampered_or_wrong_proof() {
    // A single flipped hex digit fails constant-time verification.
    let mut tampered = CLIENT_PROOF.to_string();
    tampered.replace_range(0..1, "0"); // 'f' → '0'
    assert!(!verify_client_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        &tampered
    ));
    // The server proof is not a valid client proof (role mismatch).
    assert!(!verify_client_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        SERVER_PROOF
    ));
    // A wrong token fails.
    assert!(!verify_client_proof(
        &"9".repeat(64),
        SERVER_NONCE,
        CLIENT_NONCE,
        CLIENT_PROOF
    ));
}

// ── verify_server_proof (client-side, issue #1084 PR 1) ────────────────────

#[test]
fn verify_server_proof_accepts_the_correct_proof() {
    assert!(verify_server_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        SERVER_PROOF
    ));
}

#[test]
fn verify_server_proof_rejects_a_tampered_or_wrong_proof() {
    let mut tampered = SERVER_PROOF.to_string();
    tampered.replace_range(0..1, "0");
    assert!(!verify_server_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        &tampered
    ));
    // The client proof is not a valid server proof (role mismatch) — proves
    // the two can never be swapped/replayed for each other.
    assert!(!verify_server_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        CLIENT_PROOF
    ));
    // A wrong token fails.
    assert!(!verify_server_proof(
        &"9".repeat(64),
        SERVER_NONCE,
        CLIENT_NONCE,
        SERVER_PROOF
    ));
}

#[test]
fn verify_server_proof_rejects_malformed_hex() {
    assert!(!verify_server_proof(TOKEN, SERVER_NONCE, CLIENT_NONCE, ""));
    assert!(!verify_server_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        "xyz"
    ));
}

#[test]
fn verify_rejects_malformed_hex() {
    assert!(!verify_client_proof(TOKEN, SERVER_NONCE, CLIENT_NONCE, ""));
    assert!(!verify_client_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        "xyz"
    ));
    // Odd length is not valid hex.
    assert!(!verify_client_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        "abc"
    ));
    // Uppercase is rejected by the decoder (wire proofs are lowercase).
    assert!(!verify_client_proof(
        TOKEN,
        SERVER_NONCE,
        CLIENT_NONCE,
        &CLIENT_PROOF.to_uppercase()
    ));
}

#[test]
fn nonce_shape_and_freshness() {
    let a = new_nonce();
    let b = new_nonce();
    assert_eq!(a.len(), NONCE_HEX_LEN);
    assert!(is_valid_nonce(&a));
    assert!(is_valid_nonce(&b));
    assert_ne!(a, b, "each connection gets a fresh nonce");
    // Rejects junk.
    assert!(!is_valid_nonce(""));
    assert!(!is_valid_nonce("tooshort"));
    assert!(!is_valid_nonce(&"a".repeat(NONCE_HEX_LEN + 2)));
    assert!(!is_valid_nonce("00112233445566778899AABBCCDDEEFF")); // uppercase
}
