import { describe, expect, it } from 'vitest';

import {
  ExtensionAnswersSaveRequestSchema,
  ExtensionAnswersSaveResultSchema,
  ExtensionAnswersSuggestRequestSchema,
  ExtensionAnswersSuggestResultSchema,
  ExtensionEnvelopeSchema,
} from '../extension-protocol.js';
import { EXTENSION_MESSAGE_TYPES } from '../extension-protocol-constants.js';

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

  it('accepts an auto:true request (the submit-watcher save-answers-on-submit flow, PR4)', () => {
    expect(
      ExtensionAnswersSaveRequestSchema.parse({
        url: 'https://example.com/job/123',
        answers: [{ question: 'Why this role?', answer: 'Because I love it.' }],
        auto: true,
      })
    ).toEqual({
      url: 'https://example.com/job/123',
      answers: [{ question: 'Why this role?', answer: 'Because I love it.' }],
      auto: true,
    });
  });

  it('accepts a request with no auto field at all (the ordinary click path, unchanged)', () => {
    expect(
      ExtensionAnswersSaveRequestSchema.parse({ url: 'https://example.com/job/123', answers: [] })
    ).toEqual({ url: 'https://example.com/job/123', answers: [] });
  });

  it('rejects a non-boolean auto field', () => {
    expect(() =>
      ExtensionAnswersSaveRequestSchema.parse({
        url: 'https://example.com/job/123',
        answers: [],
        auto: 'yes',
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
