import { describe, expect, it } from 'vitest';

import { ExtensionEnvelopeSchema, ExtensionImportRequestSchema } from './extension-protocol.js';
import { EXTENSION_MESSAGE_TYPES } from './extension-protocol-constants.js';

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

const VALID_ENVELOPE = {
  type: EXTENSION_MESSAGE_TYPES.importRequest,
  token: 'secret-token',
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

  it('rejects an unknown message type', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({ ...VALID_ENVELOPE, type: 'unknown.type' })
    ).toThrow();
  });

  it('rejects an envelope with an empty token', () => {
    expect(() => ExtensionEnvelopeSchema.parse({ ...VALID_ENVELOPE, token: '' })).toThrow();
  });

  it('rejects an envelope with an empty reqId', () => {
    expect(() => ExtensionEnvelopeSchema.parse({ ...VALID_ENVELOPE, reqId: '' })).toThrow();
  });

  it('rejects an envelope missing the token field', () => {
    const { token: _omit, ...noToken } = VALID_ENVELOPE;
    expect(() => ExtensionEnvelopeSchema.parse(noToken)).toThrow();
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
    // payload is z.unknown() — null is a valid unknown value
    expect(() => ExtensionEnvelopeSchema.parse({ ...VALID_ENVELOPE, payload: null })).not.toThrow();
  });
});
