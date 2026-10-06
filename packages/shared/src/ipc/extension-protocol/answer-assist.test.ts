import { describe, expect, it } from 'vitest';

import {
  ExtensionAnswerAssistRequestSchema,
  ExtensionAnswerAssistResultSchema,
  ExtensionAssistChunkPayloadSchema,
  ExtensionEnvelopeSchema,
} from '../extension-protocol.js';
import {
  EXTENSION_AI_ASSIST_OFF_MESSAGE,
  EXTENSION_ANSWER_ASSIST_MAX_CHARS,
  EXTENSION_MESSAGE_TYPES,
  EXTENSION_NO_PROVIDER_MESSAGE,
} from '../extension-protocol-constants.js';

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

  it.each(['company-brief', 'salary-answer'] as const)(
    'accepts a Prep tab topic request (%s)',
    (topic) => {
      expect(
        ExtensionAnswerAssistRequestSchema.parse({ question: 'Company brief', topic })
      ).toEqual({ question: 'Company brief', topic });
    }
  );

  it('rejects an unknown topic', () => {
    expect(() =>
      ExtensionAnswerAssistRequestSchema.parse({ question: 'Company brief', topic: 'weather' })
    ).toThrow();
  });

  it('accepts a draft request carrying the field character limit', () => {
    expect(
      ExtensionAnswerAssistRequestSchema.parse({ question: 'Why this role?', maxChars: 300 })
    ).toEqual({ question: 'Why this role?', maxChars: 300 });
  });

  it('accepts a draft request with no character limit at all', () => {
    // `maxChars` is optional in BOTH directions: an extension that never
    // reads a maxlength, and a field that has none, both send nothing.
    expect(ExtensionAnswerAssistRequestSchema.parse({ question: 'Why this role?' })).toEqual({
      question: 'Why this role?',
    });
  });

  it('accepts a maxChars far above the desktop clamp — the ceiling is not a wire bound', () => {
    // Two different bounds, and this test is the one that keeps them apart:
    //   WIRE bound      = the shape only (a positive integer). Enforced here.
    //   DESKTOP CLAMP   = EXTENSION_ANSWER_ASSIST_MAX_CHARS, applied by the
    //                     bridge's `parse_max_chars` (Rust-side test:
    //                     `clamps_an_over_large_limit_to_the_draft_cap`).
    // A schema `.max()` here would turn "quietly clamped" into "request
    // refused", so an over-large limit MUST parse. The value survives the
    // parse un-reduced precisely because the clamp does not live on the wire.
    const oversized = EXTENSION_ANSWER_ASSIST_MAX_CHARS * 10;
    expect(
      ExtensionAnswerAssistRequestSchema.parse({ question: 'Why this role?', maxChars: oversized })
    ).toEqual({ question: 'Why this role?', maxChars: oversized });
  });

  it.each([0, -1, -300, 12.5, '300', null, Number.NaN])(
    'rejects a maxChars that is not a positive integer (%s)',
    (value) => {
      expect(() =>
        ExtensionAnswerAssistRequestSchema.parse({ question: 'Why this role?', maxChars: value })
      ).toThrow();
    }
  );
});

describe('answer.assist refusal sentinels', () => {
  it('names where to turn each gate back on', () => {
    // The gated-off row matches these constants instead of copying the text;
    // the Rust parity test (`message_type_constants_match_ts`) pins them to
    // the handler's own consts, so this side only checks they stay two
    // distinct, actionable strings.
    expect(EXTENSION_AI_ASSIST_OFF_MESSAGE).toContain('Browser extension');
    expect(EXTENSION_NO_PROVIDER_MESSAGE).toContain('Settings → AI');
    expect(EXTENSION_AI_ASSIST_OFF_MESSAGE).not.toBe(EXTENSION_NO_PROVIDER_MESSAGE);
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
