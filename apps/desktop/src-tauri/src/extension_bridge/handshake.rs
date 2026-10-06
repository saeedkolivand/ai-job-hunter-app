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

use hmac::{Hmac, KeyInit, Mac};
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

/// Verify the server's step-4 `serverProof` (lowercase hex) **in constant
/// time**, mirroring [`verify_client_proof`] with [`ROLE_SERVER`] — the
/// counterpart the CLI-agent client (issue #1084 PR 1) needs and the browser
/// extension never did (its handshake lives in TS, `lib/handshake.ts`'s
/// `constantTimeHexEqual`). A malformed-hex proof is rejected the same way.
#[must_use]
pub fn verify_server_proof(
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
    mac.update(handshake_message(ROLE_SERVER, server_nonce, client_nonce).as_bytes());
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
mod tests;
