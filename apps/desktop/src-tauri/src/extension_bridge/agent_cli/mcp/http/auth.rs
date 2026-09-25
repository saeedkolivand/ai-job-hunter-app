//! The loopback bearer credential: a fresh 32-byte token per launch, compared in fixed time.
//! The one place this transport generates a secret or decides whether a caller presented it
//! correctly — no other unit may branch on the token itself.

/// A 32-byte random bearer token, lowercase hex — the same shape and generator
/// `extension_bridge::persist::new_token` uses for the pairing token (issue #1173: ">= 128 bits
/// from the OS RNG the app already uses"; 32 bytes is 256 bits, matching that sibling rather
/// than cutting a new, smaller convention).
pub(super) fn new_bearer_token() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Fixed-time comparison — a bearer token is a secret compared against caller-supplied input on
/// every request, so a length-then-byte early-exit (`==` on `&str`) would leak how many leading
/// bytes matched through response timing. `a`'s length varies only with what this process itself
/// generated, so branching on a length MISMATCH first leaks nothing about the token's content.
pub(super) fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
