//! Extension-bridge v2 mutual HMAC challenge-response — the crypto core.
//!
//! The pairing token NEVER goes on the wire in v2. Instead both sides prove they
//! know it via `HMAC-SHA256(key = token's UTF-8 bytes, msg = a canonical,
//! domain-separated string)`:
//!
//! 1. Extension → `hello { protocol: 2, clientNonce }`
//! 2. Desktop   → `challenge { serverNonce }`
//! 3. Extension → `auth { proof }` — `proof = HMAC(token, CLIENT_MSG)`
//! 4. Desktop verifies `proof` **constant-time**, then → `auth.ok { serverProof }`
//!    where `serverProof = HMAC(token, SERVER_MSG)`
//! 5. Extension verifies `serverProof` **constant-time** (mutual auth).
//!
//! The message string ([`handshake_message`]) is byte-identical to the TS side
//! (`packages/shared/src/ipc/extension-protocol-constants.ts::handshakeMessage`);
//! a shared known-answer vector pins both so the two canonicalizations can never
//! silently drift. Pure functions, no I/O, no app state — fully unit-testable.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Domain-separation prefix (bumped with the protocol version). MUST equal the TS
/// `HANDSHAKE_DOMAIN`.
pub const DOMAIN: &str = "ajh-bridge/v2";

/// The client proves in step 3.
pub const ROLE_CLIENT: &str = "client";
/// The server proves in step 4.
pub const ROLE_SERVER: &str = "server";

/// Nonce length on the wire: 16 random bytes → 32 lowercase-hex chars.
const NONCE_BYTES: usize = 16;
const NONCE_HEX_LEN: usize = NONCE_BYTES * 2;

/// Build the canonical, domain-separated message both sides HMAC. Byte-for-byte
/// identical to the TS `handshakeMessage`:
/// `ajh-bridge/v2\n<role>\n<serverNonceHex>\n<clientNonceHex>`.
pub fn handshake_message(role: &str, server_nonce: &str, client_nonce: &str) -> String {
    format!("{DOMAIN}\n{role}\n{server_nonce}\n{client_nonce}")
}

/// A fresh 16-byte CSPRNG nonce as lowercase hex (32 chars). Never reused — one
/// per connection. Mirrors [`super::new_token`]'s encoding.
pub fn new_nonce() -> String {
    use rand::Rng;
    let mut bytes = [0u8; NONCE_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    hex_encode(&bytes)
}

/// Whether `s` is a well-formed nonce we will accept from the peer: exactly
/// [`NONCE_HEX_LEN`] lowercase-hex chars. Rejects junk (wrong length, uppercase,
/// non-hex) before it reaches the HMAC.
pub fn is_valid_nonce(s: &str) -> bool {
    s.len() == NONCE_HEX_LEN
        && s.bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}

/// The client proof for step 3, lowercase hex. `key = token.as_bytes()`.
pub fn client_proof(token: &str, server_nonce: &str, client_nonce: &str) -> String {
    proof_hex(token, ROLE_CLIENT, server_nonce, client_nonce)
}

/// The server proof for step 4, lowercase hex. `key = token.as_bytes()`.
pub fn server_proof(token: &str, server_nonce: &str, client_nonce: &str) -> String {
    proof_hex(token, ROLE_SERVER, server_nonce, client_nonce)
}

/// Verify the client's step-3 `proof` (lowercase hex) **in constant time** via
/// `Mac::verify_slice` — never a `==` on the tag. A malformed-hex proof is
/// rejected (the hex is attacker-chosen, so decoding it is not a token oracle).
#[must_use]
pub fn verify_client_proof(
    token: &str,
    server_nonce: &str,
    client_nonce: &str,
    proof_hex: &str,
) -> bool {
    let Some(candidate) = hex_decode(proof_hex) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(token.as_bytes())
        .expect("HMAC-SHA256 accepts a key of any length");
    mac.update(handshake_message(ROLE_CLIENT, server_nonce, client_nonce).as_bytes());
    mac.verify_slice(&candidate).is_ok()
}

fn proof_hex(token: &str, role: &str, server_nonce: &str, client_nonce: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(token.as_bytes())
        .expect("HMAC-SHA256 accepts a key of any length");
    mac.update(handshake_message(role, server_nonce, client_nonce).as_bytes());
    hex_encode(&mac.finalize().into_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decode a lowercase-hex string to bytes, or `None` if it is not even-length
/// lowercase hex. Not constant-time by design (the input is the caller-supplied,
/// non-secret proof — only the final tag comparison must be constant-time).
fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let val = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        }
    };
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        out.push((val(pair[0])? << 4) | val(pair[1])?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
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
}
