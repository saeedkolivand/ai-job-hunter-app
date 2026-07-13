/**
 * Extension side of the v2 mutual HMAC challenge-response handshake.
 *
 * The pairing token is used ONLY as an HMAC key here — it is NEVER sent on the
 * wire. `computeProof` produces `HMAC-SHA256(key = utf8(token), msg =
 * handshakeMessage(role, serverNonce, clientNonce))` as lowercase hex, using the
 * MV3 service-worker's Web Crypto (`crypto.subtle`). The message
 * canonicalization is imported from `@ajh/shared` so it is byte-identical to the
 * Rust side; a shared known-answer vector ({@link handshake.test.ts}) pins both.
 *
 * MV3/AMO note: this uses only `crypto.subtle` + `crypto.getRandomValues`
 * (available in the SW context) and imports from the zod-free
 * `@ajh/shared/extension-protocol` — no `eval`, no dynamic import, no zod.
 */

import { handshakeMessage, type HandshakeRole } from '@ajh/shared/extension-protocol';

const encoder = new TextEncoder();

/** Nonce length on the wire: 16 random bytes → 32 lowercase-hex chars. */
const NONCE_BYTES = 16;
const NONCE_HEX_PATTERN = /^[0-9a-f]{32}$/;

/** Lowercase-hex encode a byte buffer (matches the Rust/`token` hex encoding). */
function bytesToHex(bytes: Uint8Array): string {
  let out = '';
  for (const b of bytes) out += b.toString(16).padStart(2, '0');
  return out;
}

/**
 * A fresh CSPRNG nonce as lowercase hex. 16 bytes (32 hex chars) — matches the
 * desktop's `handshake::new_nonce`. Never reuse a nonce across connections.
 */
export function randomNonceHex(bytes: number = NONCE_BYTES): string {
  const buf = new Uint8Array(bytes);
  crypto.getRandomValues(buf);
  return bytesToHex(buf);
}

/**
 * Whether `s` is a well-formed nonce as this protocol emits: exactly 32
 * lowercase-hex chars (16 bytes). Mirrors the Rust `handshake::is_valid_nonce`
 * shape check. Applied to the desktop's `serverNonce` before it feeds the HMAC —
 * defense-in-depth so a malformed/oversized peer nonce is a clean handshake
 * failure, never silently accepted into the signed message.
 */
export function isValidNonceHex(s: string): boolean {
  return NONCE_HEX_PATTERN.test(s);
}

/**
 * Compute the handshake proof for a role. `key` is the token's raw UTF-8 bytes
 * (the token is 64-char lowercase hex; its 64 ASCII bytes are the key). Returns
 * the HMAC-SHA256 as lowercase hex.
 */
export async function computeProof(
  token: string,
  role: HandshakeRole,
  serverNonce: string,
  clientNonce: string
): Promise<string> {
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(token),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign']
  );
  const message = encoder.encode(handshakeMessage(role, serverNonce, clientNonce));
  const sig = await crypto.subtle.sign('HMAC', key, message);
  return bytesToHex(new Uint8Array(sig));
}

/**
 * Constant-time equality for two lowercase-hex strings — used to verify the
 * desktop's `serverProof` without an early-return timing side channel. The
 * length check is safe (both proofs are a fixed 64 hex chars; length is not
 * secret); the byte loop never short-circuits on the first mismatch.
 */
export function constantTimeHexEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i += 1) {
    diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return diff === 0;
}
