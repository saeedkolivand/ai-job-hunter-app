import { describe, expect, it } from 'vitest';

import {
  ExtensionAppliedBatchEntrySchema,
  ExtensionAppliedCheckBatchRequestSchema,
  ExtensionAppliedCheckBatchResultSchema,
  ExtensionAppliedCheckRequestSchema,
  ExtensionAppliedCheckResultSchema,
  ExtensionEnvelopeSchema,
  ExtensionStatusUpdateRequestSchema,
  ExtensionStatusUpdateResultSchema,
} from '../extension-protocol.js';
import {
  EXTENSION_MESSAGE_TYPES,
  MAX_APPLIED_CHECK_BATCH_URLS,
} from '../extension-protocol-constants.js';

// ---------------------------------------------------------------------------
// ExtensionAppliedCheckRequestSchema / ExtensionAppliedCheckResultSchema
// ---------------------------------------------------------------------------

describe('ExtensionAppliedCheckRequestSchema', () => {
  it('accepts a minimal request', () => {
    expect(() =>
      ExtensionAppliedCheckRequestSchema.parse({ url: 'https://example.com/job/123' })
    ).not.toThrow();
  });

  it('rejects an empty url', () => {
    expect(() => ExtensionAppliedCheckRequestSchema.parse({ url: '' })).toThrow();
  });

  it('rejects a request with no url field', () => {
    expect(() => ExtensionAppliedCheckRequestSchema.parse({})).toThrow();
  });
});

describe('ExtensionAppliedCheckResultSchema', () => {
  it('accepts a not-found result (found only)', () => {
    expect(() => ExtensionAppliedCheckResultSchema.parse({ found: false })).not.toThrow();
  });

  it('round-trips a found+applied result with appliedAt', () => {
    const payload = {
      found: true,
      applicationId: 'app-1',
      status: 'applied',
      title: 'Senior Rust Engineer',
      appliedAt: 1_718_000_000_000,
    };
    expect(ExtensionAppliedCheckResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts an error payload (malformed/empty url on the desktop side)', () => {
    expect(() =>
      ExtensionAppliedCheckResultSchema.parse({ found: false, error: 'url is required' })
    ).not.toThrow();
  });

  it('rejects a missing found field', () => {
    expect(() => ExtensionAppliedCheckResultSchema.parse({})).toThrow();
  });

  it('rejects a non-boolean found field', () => {
    expect(() => ExtensionAppliedCheckResultSchema.parse({ found: 'yes' })).toThrow();
  });

  it('carries applied.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.appliedResult,
        reqId: 'req-003',
        payload: { found: false },
      })
    ).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// ExtensionAppliedCheckBatchRequestSchema / ExtensionAppliedBatchEntrySchema /
// ExtensionAppliedCheckBatchResultSchema (PR3, results-page stamps)
// ---------------------------------------------------------------------------

describe('ExtensionAppliedCheckBatchRequestSchema', () => {
  it('accepts a list of urls', () => {
    expect(() =>
      ExtensionAppliedCheckBatchRequestSchema.parse({
        urls: ['https://example.com/jobs/1', 'https://example.com/jobs/2'],
      })
    ).not.toThrow();
  });

  it('accepts an empty urls array', () => {
    expect(() => ExtensionAppliedCheckBatchRequestSchema.parse({ urls: [] })).not.toThrow();
  });

  it('rejects a request with no urls field', () => {
    expect(() => ExtensionAppliedCheckBatchRequestSchema.parse({})).toThrow();
  });

  it('rejects a non-array urls field', () => {
    expect(() =>
      ExtensionAppliedCheckBatchRequestSchema.parse({ urls: 'https://example.com/jobs/1' })
    ).toThrow();
  });

  it('accepts exactly MAX_APPLIED_CHECK_BATCH_URLS urls', () => {
    const urls = Array.from(
      { length: MAX_APPLIED_CHECK_BATCH_URLS },
      (_, i) => `https://example.com/jobs/${i}`
    );
    expect(() => ExtensionAppliedCheckBatchRequestSchema.parse({ urls })).not.toThrow();
  });

  it('rejects one more than MAX_APPLIED_CHECK_BATCH_URLS urls — the Rust side refuses `too_many_urls`, so shared validation must not pass a request it is guaranteed to reject', () => {
    const urls = Array.from(
      { length: MAX_APPLIED_CHECK_BATCH_URLS + 1 },
      (_, i) => `https://example.com/jobs/${i}`
    );
    expect(() => ExtensionAppliedCheckBatchRequestSchema.parse({ urls })).toThrow();
  });
});

describe('ExtensionAppliedBatchEntrySchema', () => {
  it('accepts an entry with a status', () => {
    expect(() =>
      ExtensionAppliedBatchEntrySchema.parse({
        url: 'https://example.com/jobs/1',
        found: true,
        status: 'saved',
      })
    ).not.toThrow();
  });

  it('accepts a not-found entry (status omitted)', () => {
    expect(() =>
      ExtensionAppliedBatchEntrySchema.parse({ url: 'https://example.com/jobs/1', found: false })
    ).not.toThrow();
  });

  it('rejects an entry missing url', () => {
    expect(() => ExtensionAppliedBatchEntrySchema.parse({ found: true })).toThrow();
  });
});

describe('ExtensionAppliedCheckBatchResultSchema', () => {
  it('round-trips a success payload, preserving order', () => {
    const payload = {
      ok: true,
      results: [
        { url: 'https://example.com/jobs/1', found: true, status: 'saved' },
        { url: 'https://example.com/jobs/2', found: false },
      ],
    };
    expect(ExtensionAppliedCheckBatchResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts a refusal (over-cap/throttle) with detail + retryAfterMs', () => {
    const payload = {
      ok: false,
      error: 'too_many_urls',
      detail: 'max 50 urls per batch',
      retryAfterMs: 2000,
    };
    expect(ExtensionAppliedCheckBatchResultSchema.parse(payload)).toEqual(payload);
  });

  it('rejects a missing ok field', () => {
    expect(() => ExtensionAppliedCheckBatchResultSchema.parse({})).toThrow();
  });

  it('rejects a contradictory ok:false payload carrying results but no error', () => {
    expect(() =>
      ExtensionAppliedCheckBatchResultSchema.parse({ ok: false, results: [] })
    ).toThrow();
  });

  it('carries applied.check.batch / applied.batch.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.appliedCheckBatch,
        reqId: 'req-020',
        payload: { urls: ['https://example.com/jobs/1'] },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.appliedBatchResult,
        reqId: 'req-020',
        payload: { ok: true, results: [] },
      })
    ).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// ExtensionStatusUpdateRequestSchema / ExtensionStatusUpdateResultSchema
// ---------------------------------------------------------------------------

describe('ExtensionStatusUpdateRequestSchema', () => {
  it('accepts a valid request', () => {
    expect(() =>
      ExtensionStatusUpdateRequestSchema.parse({
        url: 'https://example.com/job/123',
        to: 'applied',
      })
    ).not.toThrow();
  });

  it('rejects an empty url', () => {
    expect(() => ExtensionStatusUpdateRequestSchema.parse({ url: '', to: 'applied' })).toThrow();
  });

  it('rejects a request with no url field', () => {
    expect(() => ExtensionStatusUpdateRequestSchema.parse({ to: 'applied' })).toThrow();
  });

  it('rejects any `to` value other than the literal "applied" — the allowlist is visible in the contract itself', () => {
    expect(() =>
      ExtensionStatusUpdateRequestSchema.parse({ url: 'https://example.com/job/123', to: 'saved' })
    ).toThrow();
    expect(() =>
      ExtensionStatusUpdateRequestSchema.parse({
        url: 'https://example.com/job/123',
        to: 'interviewing',
      })
    ).toThrow();
  });

  it('rejects a request with no `to` field', () => {
    expect(() =>
      ExtensionStatusUpdateRequestSchema.parse({ url: 'https://example.com/job/123' })
    ).toThrow();
  });
});

describe('ExtensionStatusUpdateResultSchema', () => {
  it('round-trips a success payload', () => {
    const payload = { ok: true, applicationId: 'app-1', status: 'applied' };
    expect(ExtensionStatusUpdateResultSchema.parse(payload)).toEqual(payload);
  });

  it("accepts a user-facing failure payload (this verb's errors are shown, unlike applied.check)", () => {
    expect(() =>
      ExtensionStatusUpdateResultSchema.parse({
        ok: false,
        error: "couldn't find a saved job for this page",
      })
    ).not.toThrow();
  });

  it('rejects a missing ok field', () => {
    expect(() => ExtensionStatusUpdateResultSchema.parse({})).toThrow();
  });

  it('rejects a non-boolean ok field', () => {
    expect(() => ExtensionStatusUpdateResultSchema.parse({ ok: 'yes' })).toThrow();
  });

  it('rejects an incomplete ok:true payload missing applicationId and status', () => {
    expect(() => ExtensionStatusUpdateResultSchema.parse({ ok: true })).toThrow();
  });

  it('rejects an ok:true payload missing status', () => {
    expect(() =>
      ExtensionStatusUpdateResultSchema.parse({ ok: true, applicationId: 'app-1' })
    ).toThrow();
  });

  it('rejects an ok:true payload missing applicationId', () => {
    expect(() =>
      ExtensionStatusUpdateResultSchema.parse({ ok: true, status: 'applied' })
    ).toThrow();
  });

  it('rejects a contradictory ok:false payload carrying success fields but no error', () => {
    expect(() =>
      ExtensionStatusUpdateResultSchema.parse({
        ok: false,
        applicationId: 'app-1',
        status: 'applied',
      })
    ).toThrow();
  });

  it('carries status.update / status.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.statusUpdate,
        reqId: 'req-004',
        payload: { url: 'https://example.com/job/123', to: 'applied' },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.statusResult,
        reqId: 'req-005',
        payload: { ok: true, applicationId: 'app-1', status: 'applied' },
      })
    ).not.toThrow();
  });
});
