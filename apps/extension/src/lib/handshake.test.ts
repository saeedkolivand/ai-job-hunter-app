/**
 * Cross-implementation known-answer test for the v2 handshake HMAC.
 *
 * This asserts the extension's REAL Web Crypto (`crypto.subtle`) proof equals the
 * SAME shared vector the Rust side asserts against its `hmac` crate
 * (`apps/desktop/src-tauri/src/extension_bridge/handshake.rs::kat_*`). If the two
 * byte-canonicalizations ever drift, one side's KAT fails loudly instead of the
 * handshake silently never matching. The vector lives in `@ajh/shared` so both
 * the TS and Rust tests point at one source of truth.
 */

import { describe, expect, it } from 'vitest';

import { HANDSHAKE_TEST_VECTOR } from '@ajh/shared/extension-protocol';

import { computeProof, constantTimeHexEqual, isValidNonceHex, randomNonceHex } from './handshake';

describe('handshake HMAC — shared cross-impl known-answer vector', () => {
  const v = HANDSHAKE_TEST_VECTOR;

  it('the Web Crypto client proof matches the shared vector (byte-identical with Rust)', async () => {
    expect(await computeProof(v.token, 'client', v.serverNonce, v.clientNonce)).toBe(v.clientProof);
  });

  it('the Web Crypto server proof matches the shared vector (byte-identical with Rust)', async () => {
    expect(await computeProof(v.token, 'server', v.serverNonce, v.clientNonce)).toBe(v.serverProof);
  });

  it('the client and server proofs are domain-separated (role changes the proof)', async () => {
    const client = await computeProof(v.token, 'client', v.serverNonce, v.clientNonce);
    const server = await computeProof(v.token, 'server', v.serverNonce, v.clientNonce);
    expect(client).not.toBe(server);
  });
});

describe('constantTimeHexEqual', () => {
  it('accepts equal strings and rejects any difference (content or length)', () => {
    expect(constantTimeHexEqual('deadbeef', 'deadbeef')).toBe(true);
    expect(constantTimeHexEqual('deadbeef', 'deadbeff')).toBe(false);
    expect(constantTimeHexEqual('deadbeef', 'deadbee')).toBe(false);
    expect(constantTimeHexEqual('', '')).toBe(true);
  });
});

describe('randomNonceHex', () => {
  it('returns 32 lowercase-hex chars (16 bytes), fresh per call', () => {
    const a = randomNonceHex();
    const b = randomNonceHex();
    expect(a).toMatch(/^[0-9a-f]{32}$/);
    expect(b).toMatch(/^[0-9a-f]{32}$/);
    expect(a).not.toBe(b);
  });

  it('every generated nonce passes isValidNonceHex (round-trip with the validator)', () => {
    expect(isValidNonceHex(randomNonceHex())).toBe(true);
  });
});

describe('isValidNonceHex', () => {
  it('accepts exactly 32 lowercase-hex chars', () => {
    expect(isValidNonceHex('00112233445566778899aabbccddeeff'.slice(0, 32))).toBe(true);
    expect(isValidNonceHex('ffeeddccbbaa99887766554433221100')).toBe(true);
  });

  it('rejects wrong length, uppercase, and non-hex — mirrors the Rust is_valid_nonce shape check', () => {
    expect(isValidNonceHex('')).toBe(false);
    expect(isValidNonceHex('tooshort')).toBe(false);
    expect(isValidNonceHex('a'.repeat(31))).toBe(false); // one short
    expect(isValidNonceHex('a'.repeat(33))).toBe(false); // one over
    expect(isValidNonceHex('00112233445566778899AABBCCDDEEFF')).toBe(false); // uppercase
    expect(isValidNonceHex('not-hex!!'.padEnd(32, '0'))).toBe(false); // non-hex chars
  });
});
