import { describe, expect, it } from 'vitest';

import {
  ExtensionAnswerAssistRequestSchema,
  ExtensionAnswerAssistResultSchema,
  ExtensionAnswersSaveRequestSchema,
  ExtensionAnswersSaveResultSchema,
  ExtensionAnswersSuggestRequestSchema,
  ExtensionAnswersSuggestResultSchema,
  ExtensionAppliedCheckRequestSchema,
  ExtensionAppliedCheckResultSchema,
  ExtensionAssistChunkPayloadSchema,
  ExtensionEnvelopeSchema,
  ExtensionImportRequestSchema,
  ExtensionMatchLiveRequestSchema,
  ExtensionMatchLiveResultSchema,
  ExtensionProfileResultSchema,
  ExtensionStatusUpdateRequestSchema,
  ExtensionStatusUpdateResultSchema,
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

  it('accepts a profile with extraLinks', () => {
    const payload = {
      email: 'x@y.z',
      extraLinks: [
        { label: 'Portfolio', url: 'https://saeed.dev' },
        { label: 'Dribbble', url: 'https://dribbble.com/saeed' },
      ],
    };
    expect(ExtensionProfileResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts a profile with no extraLinks field (additive — old replies omit it)', () => {
    expect(() => ExtensionProfileResultSchema.parse({ email: 'x@y.z' })).not.toThrow();
    const parsed = ExtensionProfileResultSchema.parse({ email: 'x@y.z' });
    expect(parsed.extraLinks).toBeUndefined();
  });

  it('rejects a malformed extraLinks entry (missing url)', () => {
    expect(() =>
      ExtensionProfileResultSchema.parse({ extraLinks: [{ label: 'Portfolio' }] })
    ).toThrow();
  });

  it('rejects a malformed extraLinks entry (non-string label)', () => {
    expect(() =>
      ExtensionProfileResultSchema.parse({
        extraLinks: [{ label: 42, url: 'https://saeed.dev' }],
      })
    ).toThrow();
  });

  it('rejects a non-array extraLinks', () => {
    expect(() => ExtensionProfileResultSchema.parse({ extraLinks: 'https://saeed.dev' })).toThrow();
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

// ---------------------------------------------------------------------------
// ExtensionAnswersSaveRequestSchema / ExtensionAnswersSaveResultSchema
// ---------------------------------------------------------------------------

describe('ExtensionAnswersSaveRequestSchema', () => {
  it('accepts a valid request with captured pairs', () => {
    expect(() =>
      ExtensionAnswersSaveRequestSchema.parse({
        url: 'https://example.com/job/123',
        answers: [{ question: 'Why this role?', answer: 'Because I love it.' }],
      })
    ).not.toThrow();
  });

  it('accepts an empty answers array', () => {
    expect(() =>
      ExtensionAnswersSaveRequestSchema.parse({ url: 'https://example.com/job/123', answers: [] })
    ).not.toThrow();
  });

  it('rejects an empty url', () => {
    expect(() => ExtensionAnswersSaveRequestSchema.parse({ url: '', answers: [] })).toThrow();
  });

  it('rejects a request with no url field', () => {
    expect(() => ExtensionAnswersSaveRequestSchema.parse({ answers: [] })).toThrow();
  });

  it('rejects a request with no answers field', () => {
    expect(() =>
      ExtensionAnswersSaveRequestSchema.parse({ url: 'https://example.com/job/123' })
    ).toThrow();
  });

  it('rejects a malformed answer entry (missing answer)', () => {
    expect(() =>
      ExtensionAnswersSaveRequestSchema.parse({
        url: 'https://example.com/job/123',
        answers: [{ question: 'Why this role?' }],
      })
    ).toThrow();
  });

  it('rejects a non-array answers field', () => {
    expect(() =>
      ExtensionAnswersSaveRequestSchema.parse({
        url: 'https://example.com/job/123',
        answers: 'not-an-array',
      })
    ).toThrow();
  });
});

describe('ExtensionAnswersSaveResultSchema', () => {
  it('round-trips a success payload with title/company', () => {
    const payload = {
      ok: true,
      applicationId: 'app-1',
      saved: 3,
      skipped: 1,
      title: 'Backend Engineer',
      company: 'Acme',
    };
    expect(ExtensionAnswersSaveResultSchema.parse(payload)).toEqual(payload);
  });

  it('round-trips a success payload without title/company (both optional)', () => {
    const payload = { ok: true, applicationId: 'app-1', saved: 0, skipped: 0 };
    expect(ExtensionAnswersSaveResultSchema.parse(payload)).toEqual(payload);
  });

  it("accepts a user-facing failure payload (this verb's errors are shown, unlike applied.check)", () => {
    expect(() =>
      ExtensionAnswersSaveResultSchema.parse({
        ok: false,
        error: "couldn't find a saved job for this page — import it first",
      })
    ).not.toThrow();
  });

  it('rejects a missing ok field', () => {
    expect(() => ExtensionAnswersSaveResultSchema.parse({})).toThrow();
  });

  it('rejects a non-boolean ok field', () => {
    expect(() => ExtensionAnswersSaveResultSchema.parse({ ok: 'yes' })).toThrow();
  });

  it('rejects an incomplete ok:true payload missing saved/skipped', () => {
    expect(() =>
      ExtensionAnswersSaveResultSchema.parse({ ok: true, applicationId: 'app-1' })
    ).toThrow();
  });

  it('rejects a contradictory ok:false payload carrying success fields but no error', () => {
    expect(() =>
      ExtensionAnswersSaveResultSchema.parse({
        ok: false,
        applicationId: 'app-1',
        saved: 1,
        skipped: 0,
      })
    ).toThrow();
  });

  it('carries answers.save / answers.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.answersSave,
        reqId: 'req-006',
        payload: { url: 'https://example.com/job/123', answers: [] },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.answersResult,
        reqId: 'req-007',
        payload: { ok: true, applicationId: 'app-1', saved: 1, skipped: 0 },
      })
    ).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// ExtensionAnswersSuggestRequestSchema / ExtensionAnswersSuggestResultSchema
// ---------------------------------------------------------------------------

describe('ExtensionAnswersSuggestRequestSchema', () => {
  it('accepts a valid request with questions', () => {
    expect(() =>
      ExtensionAnswersSuggestRequestSchema.parse({
        questions: ['Why this role?', 'What is your notice period?'],
      })
    ).not.toThrow();
  });

  it('accepts an empty questions array', () => {
    expect(() => ExtensionAnswersSuggestRequestSchema.parse({ questions: [] })).not.toThrow();
  });

  it('rejects a request with no questions field', () => {
    expect(() => ExtensionAnswersSuggestRequestSchema.parse({})).toThrow();
  });

  it('rejects a non-array questions field', () => {
    expect(() =>
      ExtensionAnswersSuggestRequestSchema.parse({ questions: 'not-an-array' })
    ).toThrow();
  });

  it('rejects a non-string entry', () => {
    expect(() => ExtensionAnswersSuggestRequestSchema.parse({ questions: [42] })).toThrow();
  });
});

describe('ExtensionAnswersSuggestResultSchema', () => {
  it('round-trips a success payload with a full suggestion', () => {
    const payload = {
      ok: true,
      suggestions: [
        {
          question: 'Why this role?',
          answer: 'Because I love it.',
          sourceCompany: 'Acme',
          sourceTitle: 'Backend Engineer',
          sourceQuestion: 'Why this role?',
          score: 0.8,
          salary: false,
        },
      ],
    };
    expect(ExtensionAnswersSuggestResultSchema.parse(payload)).toEqual(payload);
  });

  it('round-trips a success payload with an empty suggestions array', () => {
    const payload = { ok: true, suggestions: [] };
    expect(ExtensionAnswersSuggestResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts a suggestion without sourceCompany/sourceTitle (both optional)', () => {
    expect(() =>
      ExtensionAnswersSuggestResultSchema.parse({
        ok: true,
        suggestions: [
          {
            question: 'Why this role?',
            answer: 'Because I love it.',
            sourceQuestion: 'Why this role?',
            score: 0.6,
            salary: false,
          },
        ],
      })
    ).not.toThrow();
  });

  it('rejects a suggestion missing the required sourceQuestion field', () => {
    expect(() =>
      ExtensionAnswersSuggestResultSchema.parse({
        ok: true,
        suggestions: [
          { question: 'Why this role?', answer: 'Because I love it.', score: 0.6, salary: false },
        ],
      })
    ).toThrow();
  });

  it("accepts a user-facing failure payload (this verb's errors are shown, unlike applied.check)", () => {
    expect(() =>
      ExtensionAnswersSuggestResultSchema.parse({ ok: false, error: 'Autofill is off.' })
    ).not.toThrow();
  });

  it('rejects a missing ok field', () => {
    expect(() => ExtensionAnswersSuggestResultSchema.parse({})).toThrow();
  });

  it('rejects an incomplete ok:true payload missing suggestions', () => {
    expect(() => ExtensionAnswersSuggestResultSchema.parse({ ok: true })).toThrow();
  });

  it('rejects a suggestion missing the required salary field', () => {
    expect(() =>
      ExtensionAnswersSuggestResultSchema.parse({
        ok: true,
        suggestions: [{ question: 'Why this role?', answer: 'Because I love it.', score: 0.6 }],
      })
    ).toThrow();
  });

  it('rejects a contradictory ok:false payload carrying success fields but no error', () => {
    expect(() =>
      ExtensionAnswersSuggestResultSchema.parse({ ok: false, suggestions: [] })
    ).toThrow();
  });

  it('carries answers.suggest / answers.suggest.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.answersSuggest,
        reqId: 'req-008',
        payload: { questions: ['Why this role?'] },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.answersSuggestResult,
        reqId: 'req-009',
        payload: { ok: true, suggestions: [] },
      })
    ).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// ExtensionAnswerAssistRequestSchema / ExtensionAnswerAssistResultSchema
// ---------------------------------------------------------------------------

describe('ExtensionAnswerAssistRequestSchema', () => {
  it('accepts a minimal request (question only)', () => {
    expect(() =>
      ExtensionAnswerAssistRequestSchema.parse({ question: 'Why do you want this role?' })
    ).not.toThrow();
  });

  it('accepts a full request with url and searchWeb', () => {
    expect(() =>
      ExtensionAnswerAssistRequestSchema.parse({
        question: 'What are your salary expectations?',
        url: 'https://example.com/job/123',
        searchWeb: true,
      })
    ).not.toThrow();
  });

  it('rejects an empty question', () => {
    expect(() => ExtensionAnswerAssistRequestSchema.parse({ question: '' })).toThrow();
  });

  it('rejects a request with no question field', () => {
    expect(() => ExtensionAnswerAssistRequestSchema.parse({})).toThrow();
  });

  it('accepts a rewrite-mode request (existingAnswer + preset)', () => {
    expect(() =>
      ExtensionAnswerAssistRequestSchema.parse({
        question: 'Why this role?',
        mode: 'rewrite',
        existingAnswer: 'Because I like it.',
        preset: 'shorten',
      })
    ).not.toThrow();
  });

  it('accepts a rewrite-mode request with a free-text instruction instead of a preset', () => {
    expect(() =>
      ExtensionAnswerAssistRequestSchema.parse({
        question: 'Why this role?',
        mode: 'rewrite',
        existingAnswer: 'Because I like it.',
        instruction: 'Make this sound more confident.',
      })
    ).not.toThrow();
  });

  it('rejects an unknown preset id', () => {
    expect(() =>
      ExtensionAnswerAssistRequestSchema.parse({
        question: 'Why this role?',
        mode: 'rewrite',
        existingAnswer: 'x',
        preset: 'summarize',
      })
    ).toThrow();
  });

  it('rejects an unknown mode', () => {
    expect(() =>
      ExtensionAnswerAssistRequestSchema.parse({ question: 'Why this role?', mode: 'edit' })
    ).toThrow();
  });
});

describe('ExtensionAnswerAssistResultSchema', () => {
  it('round-trips a success payload', () => {
    const payload = {
      ok: true,
      question: 'Why do you want this role?',
      draft: 'I am drawn to this role because…',
      sourced: { web: false, brief: true, salary: false },
    };
    expect(ExtensionAnswerAssistResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts an ok:true payload with an empty sourced object (no optional context used)', () => {
    expect(() =>
      ExtensionAnswerAssistResultSchema.parse({
        ok: true,
        question: 'Why this role?',
        draft: 'Because…',
        sourced: {},
      })
    ).not.toThrow();
  });

  it("accepts a user-facing failure payload (this verb's errors are shown, like status.update)", () => {
    expect(() =>
      ExtensionAnswerAssistResultSchema.parse({
        ok: false,
        error: 'AI answer drafting is off.',
      })
    ).not.toThrow();
  });

  it('rejects a missing ok field', () => {
    expect(() => ExtensionAnswerAssistResultSchema.parse({})).toThrow();
  });

  it('rejects an incomplete ok:true payload missing draft', () => {
    expect(() =>
      ExtensionAnswerAssistResultSchema.parse({
        ok: true,
        question: 'Why this role?',
        sourced: {},
      })
    ).toThrow();
  });

  it('rejects a contradictory ok:false payload carrying success fields but no error', () => {
    expect(() => ExtensionAnswerAssistResultSchema.parse({ ok: false, draft: 'x' })).toThrow();
  });

  it('carries answer.assist / answer.assist.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.answerAssist,
        reqId: 'req-012',
        payload: { question: 'Why this role?' },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.answerAssistResult,
        reqId: 'req-013',
        payload: { ok: true, question: 'Why this role?', draft: 'Because…', sourced: {} },
      })
    ).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// ExtensionAssistChunkPayloadSchema / assist.chunk / assist.done / assist.cancel
// ---------------------------------------------------------------------------

describe('ExtensionAssistChunkPayloadSchema', () => {
  it('accepts a delta string', () => {
    expect(() => ExtensionAssistChunkPayloadSchema.parse({ delta: 'Because I ' })).not.toThrow();
  });

  it('accepts an empty delta', () => {
    expect(() => ExtensionAssistChunkPayloadSchema.parse({ delta: '' })).not.toThrow();
  });

  it('rejects a missing delta', () => {
    expect(() => ExtensionAssistChunkPayloadSchema.parse({})).toThrow();
  });

  it('rejects a non-string delta', () => {
    expect(() => ExtensionAssistChunkPayloadSchema.parse({ delta: 42 })).toThrow();
  });
});

describe('assist.chunk / assist.done / assist.cancel envelopes', () => {
  it('carries assist.chunk through a valid envelope, correlated by reqId', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.assistChunk,
        reqId: 'req-020',
        payload: { delta: 'Because I ' },
      })
    ).not.toThrow();
  });

  it('carries assist.done / assist.cancel with a null (no-op) payload', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.assistDone,
        reqId: 'req-020',
        payload: null,
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.assistCancel,
        reqId: 'req-020',
        payload: null,
      })
    ).not.toThrow();
  });
});

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
