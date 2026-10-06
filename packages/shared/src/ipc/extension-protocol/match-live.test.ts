import { describe, expect, it } from 'vitest';

import {
  ExtensionEnvelopeSchema,
  ExtensionMatchLiveRequestSchema,
  ExtensionMatchLiveResultSchema,
} from '../extension-protocol.js';
import { EXTENSION_MESSAGE_TYPES } from '../extension-protocol-constants.js';

// ---------------------------------------------------------------------------
// ExtensionMatchLiveRequestSchema / ExtensionMatchLiveResultSchema
// ---------------------------------------------------------------------------

describe('ExtensionMatchLiveRequestSchema', () => {
  it('accepts a valid request with url and html', () => {
    expect(() =>
      ExtensionMatchLiveRequestSchema.parse({
        url: 'https://example.com/job/123',
        html: '<html>...</html>',
      })
    ).not.toThrow();
  });

  it('rejects a request with no html field (no URL-mode fallback for this verb)', () => {
    expect(() =>
      ExtensionMatchLiveRequestSchema.parse({ url: 'https://example.com/job/123' })
    ).toThrow();
  });

  it('rejects an empty html field', () => {
    expect(() =>
      ExtensionMatchLiveRequestSchema.parse({ url: 'https://example.com/job/123', html: '' })
    ).toThrow();
  });

  it('rejects an empty url', () => {
    expect(() =>
      ExtensionMatchLiveRequestSchema.parse({ url: '', html: '<html></html>' })
    ).toThrow();
  });
});

describe('ExtensionMatchLiveResultSchema', () => {
  it('round-trips a success payload', () => {
    const payload = {
      ok: true,
      combined: 72,
      ats: 60,
      gaps: ['kubernetes', 'terraform'],
      resumeName: 'My Resume',
      scoreSource: 'keyword',
    };
    expect(ExtensionMatchLiveResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts the wire-reserved optional semantic field', () => {
    expect(() =>
      ExtensionMatchLiveResultSchema.parse({
        ok: true,
        combined: 72,
        ats: 60,
        semantic: 80,
        gaps: [],
        resumeName: 'My Resume',
        scoreSource: 'combined',
      })
    ).not.toThrow();
  });

  it("accepts a user-facing failure payload (this verb's errors are shown, like status.update)", () => {
    expect(() =>
      ExtensionMatchLiveResultSchema.parse({
        ok: false,
        error: 'Add a resume in AI Job Hunter first, then try Check fit again.',
      })
    ).not.toThrow();
  });

  it('accepts the optional PR3 salary object (two verbatim facts, never a verdict)', () => {
    const payload = {
      ok: true,
      combined: 72,
      ats: 60,
      gaps: [],
      resumeName: 'My Resume',
      scoreSource: 'keyword',
      salary: { posting: '€70,000–€90,000', expectation: '€80,000' },
    };
    expect(ExtensionMatchLiveResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts salary with posting only (expectation is optional)', () => {
    expect(() =>
      ExtensionMatchLiveResultSchema.parse({
        ok: true,
        combined: 72,
        ats: 60,
        gaps: [],
        resumeName: 'My Resume',
        scoreSource: 'keyword',
        salary: { posting: '$100k–$120k' },
      })
    ).not.toThrow();
  });

  it('rejects a salary object missing posting', () => {
    expect(() =>
      ExtensionMatchLiveResultSchema.parse({
        ok: true,
        combined: 72,
        ats: 60,
        gaps: [],
        resumeName: 'My Resume',
        scoreSource: 'keyword',
        salary: { expectation: '€80,000' },
      })
    ).toThrow();
  });

  it('rejects a missing ok field', () => {
    expect(() => ExtensionMatchLiveResultSchema.parse({})).toThrow();
  });

  it('rejects an ok:true payload with an invalid scoreSource literal', () => {
    expect(() =>
      ExtensionMatchLiveResultSchema.parse({
        ok: true,
        combined: 10,
        ats: 10,
        gaps: [],
        resumeName: 'r',
        scoreSource: 'bogus',
      })
    ).toThrow();
  });

  it('rejects an incomplete ok:true payload missing resumeName', () => {
    expect(() =>
      ExtensionMatchLiveResultSchema.parse({
        ok: true,
        combined: 10,
        ats: 10,
        gaps: [],
        scoreSource: 'keyword',
      })
    ).toThrow();
  });

  it('rejects a contradictory ok:false payload carrying success fields but no error', () => {
    expect(() =>
      ExtensionMatchLiveResultSchema.parse({ ok: false, combined: 10, ats: 10 })
    ).toThrow();
  });

  it('carries match.live / match.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.matchLive,
        reqId: 'req-010',
        payload: { url: 'https://example.com/job/123', html: '<html></html>' },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.matchResult,
        reqId: 'req-011',
        payload: {
          ok: true,
          combined: 72,
          ats: 60,
          gaps: [],
          resumeName: 'My Resume',
          scoreSource: 'keyword',
        },
      })
    ).not.toThrow();
  });
});
