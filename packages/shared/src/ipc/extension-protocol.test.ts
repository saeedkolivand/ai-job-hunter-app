import { describe, expect, it } from 'vitest';

import {
  ExtensionEnvelopeSchema,
  ExtensionImportRequestSchema,
  ExtensionProfileResultSchema,
} from './extension-protocol.js';
import {
  EXTENSION_MESSAGE_TYPES,
  HANDSHAKE_TEST_VECTOR,
  handshakeMessage,
} from './extension-protocol-constants.js';

// ---------------------------------------------------------------------------
// ExtensionImportRequestSchema
// ---------------------------------------------------------------------------

describe('ExtensionImportRequestSchema', () => {
  it('accepts a minimal URL-mode request', () => {
    expect(() =>
      ExtensionImportRequestSchema.parse({ url: 'https://example.com/job/123' })
    ).not.toThrow();
  });

  it('accepts a Scan-mode request with html and applied flag', () => {
    expect(() =>
      ExtensionImportRequestSchema.parse({
        url: 'https://linkedin.com/jobs/view/123',
        html: '<html>...</html>',
        applied: true,
      })
    ).not.toThrow();
  });

  it('rejects a request with an empty url', () => {
    expect(() => ExtensionImportRequestSchema.parse({ url: '' })).toThrow();
  });

  it('rejects a request with no url field', () => {
    expect(() => ExtensionImportRequestSchema.parse({})).toThrow();
  });

  it('rejects a request where url is not a string', () => {
    expect(() => ExtensionImportRequestSchema.parse({ url: 42 })).toThrow();
  });
});

// ---------------------------------------------------------------------------
// ExtensionEnvelopeSchema
// ---------------------------------------------------------------------------

// v2 envelope: NO `token` field — the handshake authenticates the socket, so no
// frame carries the pairing secret.
const VALID_ENVELOPE = {
  type: EXTENSION_MESSAGE_TYPES.importRequest,
  reqId: 'req-001',
  payload: { url: 'https://example.com/job/1' },
} as const;

describe('ExtensionEnvelopeSchema', () => {
  it('accepts a valid import.request envelope', () => {
    expect(() => ExtensionEnvelopeSchema.parse(VALID_ENVELOPE)).not.toThrow();
  });

  it('accepts each known message type in the type discriminator', () => {
    for (const type of Object.values(EXTENSION_MESSAGE_TYPES)) {
      expect(() => ExtensionEnvelopeSchema.parse({ ...VALID_ENVELOPE, type })).not.toThrow();
    }
  });

  it('accepts the v2 handshake frames (hello / challenge / auth / auth.ok)', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.hello,
        reqId: 'h1',
        payload: { protocol: 2, clientNonce: 'abcd' },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.authOk,
        reqId: 'a1',
        payload: { serverProof: 'deadbeef' },
      })
    ).not.toThrow();
  });

  it('rejects an unknown message type', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({ ...VALID_ENVELOPE, type: 'unknown.type' })
    ).toThrow();
  });

  it('ignores a stray token field (v2 envelopes are token-free)', () => {
    // `z.object()`'s default behavior is to STRIP unknown keys (no `.strict()`/
    // `.passthrough()` on ExtensionEnvelopeSchema) — verified empirically, not
    // assumed. A frame that still carries a `token` (e.g. a stale caller that
    // hasn't migrated) parses successfully AND the key is dropped from the
    // output — the schema no longer requires OR echoes a token; the mutual
    // handshake is what actually authenticates, never a per-frame secret.
    const withStrayToken = { ...VALID_ENVELOPE, token: 'stale-v1-token' };
    const parsed = ExtensionEnvelopeSchema.parse(withStrayToken);
    expect(parsed).not.toHaveProperty('token');
  });

  it('rejects an envelope with an empty reqId', () => {
    expect(() => ExtensionEnvelopeSchema.parse({ ...VALID_ENVELOPE, reqId: '' })).toThrow();
  });

  it('rejects an envelope missing the reqId field', () => {
    const { reqId: _omit, ...noReqId } = VALID_ENVELOPE;
    expect(() => ExtensionEnvelopeSchema.parse(noReqId)).toThrow();
  });

  it('rejects an envelope missing the type field', () => {
    const { type: _omit, ...noType } = VALID_ENVELOPE;
    expect(() => ExtensionEnvelopeSchema.parse(noType)).toThrow();
  });

  it('accepts an envelope with null payload (unknown field accepts anything)', () => {
    // payload is intentionally z.unknown() — validation is deferred to the message-type-specific
    // handler, so the envelope schema accepts any payload shape (including null) without failing.
    expect(() => ExtensionEnvelopeSchema.parse({ ...VALID_ENVELOPE, payload: null })).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// Handshake message canonicalization (byte-identical with the Rust side)
// ---------------------------------------------------------------------------

describe('handshakeMessage', () => {
  it('builds the exact domain-separated, newline-delimited canonical string', () => {
    const { clientNonce, serverNonce } = HANDSHAKE_TEST_VECTOR;
    expect(handshakeMessage('client', serverNonce, clientNonce)).toBe(
      `ajh-bridge/v2\nclient\n${serverNonce}\n${clientNonce}`
    );
    expect(handshakeMessage('server', serverNonce, clientNonce)).toBe(
      `ajh-bridge/v2\nserver\n${serverNonce}\n${clientNonce}`
    );
  });
});

// ---------------------------------------------------------------------------
// ExtensionProfileResultSchema (assisted-autofill profile.result payload)
// ---------------------------------------------------------------------------

describe('ExtensionProfileResultSchema', () => {
  it('round-trips a full profile payload', () => {
    const payload = {
      fullName: 'Saeed Kolivand',
      email: 'saeed@example.com',
      phone: '+31 6 1234 5678',
      location: 'Amsterdam, Netherlands',
      linkedin: 'https://linkedin.com/in/saeed',
      github: 'https://github.com/saeed',
      website: 'https://saeed.dev',
    };
    expect(ExtensionProfileResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts a sparse profile (every field optional)', () => {
    expect(() => ExtensionProfileResultSchema.parse({ email: 'x@y.z' })).not.toThrow();
    expect(() => ExtensionProfileResultSchema.parse({})).not.toThrow();
  });

  it('accepts a refusal payload carrying only an error', () => {
    expect(() =>
      ExtensionProfileResultSchema.parse({ error: 'autofill is disabled' })
    ).not.toThrow();
  });

  it('rejects a non-string field', () => {
    expect(() => ExtensionProfileResultSchema.parse({ email: 42 })).toThrow();
  });

  it('carries profile.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.profileResult,
        reqId: 'req-002',
        payload: { email: 'x@y.z' },
      })
    ).not.toThrow();
  });
});
