import { describe, expect, it } from 'vitest';

import {
  AgentConfirmRequestSchema,
  AgentRunRequestSchema,
  AiGenerateRequestSchema,
  AutopilotCreateSchema,
  BOARD_IDS,
  LocaleSchema,
  ScrapeBoardsRequestSchema,
} from './index';

describe('LocaleSchema', () => {
  it('accepts all supported locales', () => {
    const supported = ['en', 'de', 'fr', 'es', 'it', 'tr', 'pt', 'ru', 'zh', 'ja', 'ko'];
    for (const locale of supported) {
      expect(() => LocaleSchema.parse(locale)).not.toThrow();
    }
  });

  it('rejects unknown locale', () => {
    expect(() => LocaleSchema.parse('xx')).toThrow();
    expect(() => LocaleSchema.parse('')).toThrow();
    expect(() => LocaleSchema.parse(null)).toThrow();
  });
});

describe('AgentRunRequestSchema', () => {
  const valid = {
    resumeId: 'res-1',
    jobId: 'job-1',
  };

  it('accepts a valid request', () => {
    expect(() => AgentRunRequestSchema.parse(valid)).not.toThrow();
  });

  it('requires resumeId and jobId (each non-empty)', () => {
    for (const key of ['resumeId', 'jobId'] as const) {
      const { [key]: _omitted, ...without } = valid;
      expect(() => AgentRunRequestSchema.parse(without)).toThrow();
      expect(() => AgentRunRequestSchema.parse({ ...valid, [key]: '' })).toThrow();
    }
  });

  // Security lock (task #25): routing is backend-owned. Even if a compromised
  // renderer sends provider/model/baseUrl, the schema strips them so nothing can
  // point a credentialed agent request at an attacker endpoint — the wire contract
  // simply has no field to carry them.
  it('strips renderer-supplied routing fields (provider/model/baseUrl are no longer wire inputs)', () => {
    const parsed = AgentRunRequestSchema.parse({
      ...valid,
      provider: 'openai-compatible',
      model: 'evil',
      baseUrl: 'http://attacker.example',
    });
    expect(parsed).toEqual(valid);
  });
});

describe('AgentConfirmRequestSchema', () => {
  const valid = { jobId: 'job-1', callId: '3-save_cover_letter', decision: 'approve' as const };

  it('accepts each valid decision, with optional editedArgs', () => {
    for (const decision of ['approve', 'approveEdited', 'deny'] as const) {
      expect(() => AgentConfirmRequestSchema.parse({ ...valid, decision })).not.toThrow();
    }
    expect(() =>
      AgentConfirmRequestSchema.parse({
        ...valid,
        decision: 'approveEdited',
        editedArgs: { coverLetterText: 'edited' },
      })
    ).not.toThrow();
  });

  it('requires non-empty jobId + callId and a known decision', () => {
    expect(() => AgentConfirmRequestSchema.parse({ ...valid, jobId: '' })).toThrow();
    expect(() => AgentConfirmRequestSchema.parse({ ...valid, callId: '' })).toThrow();
    expect(() => AgentConfirmRequestSchema.parse({ ...valid, decision: 'nuke' })).toThrow();
    const { jobId: _j, ...noJob } = valid;
    expect(() => AgentConfirmRequestSchema.parse(noJob)).toThrow();
  });
});

describe('AiGenerateRequestSchema', () => {
  const valid = {
    model: 'llama3',
    messages: [{ role: 'user', content: 'hello' }],
    locale: 'en',
  };

  it('accepts a valid request', () => {
    expect(() => AiGenerateRequestSchema.parse(valid)).not.toThrow();
  });

  it('requires locale', () => {
    const { locale: _, ...without } = valid;
    expect(() => AiGenerateRequestSchema.parse(without)).toThrow();
  });

  it('requires at least one message', () => {
    expect(() => AiGenerateRequestSchema.parse({ ...valid, messages: [] })).toThrow();
  });

  it('rejects empty model', () => {
    expect(() => AiGenerateRequestSchema.parse({ ...valid, model: '' })).toThrow();
  });

  it('clamps temperature to 0–2', () => {
    expect(() => AiGenerateRequestSchema.parse({ ...valid, temperature: 3 })).toThrow();
    expect(() => AiGenerateRequestSchema.parse({ ...valid, temperature: -0.1 })).toThrow();
    expect(() => AiGenerateRequestSchema.parse({ ...valid, temperature: 1.5 })).not.toThrow();
  });

  it('accepts every sampling param unset (all optional)', () => {
    expect(() => AiGenerateRequestSchema.parse(valid)).not.toThrow();
  });

  it('clamps topP to 0–1', () => {
    expect(() => AiGenerateRequestSchema.parse({ ...valid, topP: -0.1 })).toThrow();
    expect(() => AiGenerateRequestSchema.parse({ ...valid, topP: 1.1 })).toThrow();
    expect(() => AiGenerateRequestSchema.parse({ ...valid, topP: 0.95 })).not.toThrow();
  });

  it('clamps frequencyPenalty and presencePenalty to -2–2', () => {
    for (const key of ['frequencyPenalty', 'presencePenalty'] as const) {
      expect(() => AiGenerateRequestSchema.parse({ ...valid, [key]: -2.1 })).toThrow();
      expect(() => AiGenerateRequestSchema.parse({ ...valid, [key]: 2.1 })).toThrow();
      expect(() => AiGenerateRequestSchema.parse({ ...valid, [key]: 0.3 })).not.toThrow();
    }
  });

  it('clamps repeatPenalty to 1–2 (Ollama semantics)', () => {
    expect(() => AiGenerateRequestSchema.parse({ ...valid, repeatPenalty: 0.9 })).toThrow();
    expect(() => AiGenerateRequestSchema.parse({ ...valid, repeatPenalty: 2.1 })).toThrow();
    expect(() => AiGenerateRequestSchema.parse({ ...valid, repeatPenalty: 1.15 })).not.toThrow();
  });
});

describe('AutopilotCreateSchema', () => {
  const valid = {
    name: 'My autopilot',
    target: { boards: ['linkedin'], query: 'react developer', pages: 2 },
    filter: { minMatchScore: 60 },
    schedule: 'daily',
  };

  it('accepts a valid autopilot', () => {
    expect(() => AutopilotCreateSchema.parse(valid)).not.toThrow();
  });

  it('rejects empty name', () => {
    expect(() => AutopilotCreateSchema.parse({ ...valid, name: '' })).toThrow();
  });

  it('rejects invalid schedule', () => {
    expect(() => AutopilotCreateSchema.parse({ ...valid, schedule: 'weekly' })).toThrow();
  });

  it('clamps minMatchScore to 0–100', () => {
    expect(() =>
      AutopilotCreateSchema.parse({
        ...valid,
        filter: { minMatchScore: 150 },
      })
    ).toThrow();
  });

  it('rejects workType outside enum', () => {
    expect(() =>
      AutopilotCreateSchema.parse({
        ...valid,
        target: { ...valid.target, workType: 'freelance' },
      })
    ).toThrow();
  });

  it('accepts valid workType values', () => {
    for (const wt of ['remote', 'hybrid', 'on-site']) {
      expect(() =>
        AutopilotCreateSchema.parse({
          ...valid,
          target: { ...valid.target, workType: wt },
        })
      ).not.toThrow();
    }
  });

  it('accepts scheduleHour boundary values 0 and 23', () => {
    expect(AutopilotCreateSchema.safeParse({ ...valid, scheduleHour: 0 }).success).toBe(true);
    expect(AutopilotCreateSchema.safeParse({ ...valid, scheduleHour: 23 }).success).toBe(true);
  });

  it('accepts scheduleMinute boundary values 0 and 59', () => {
    expect(AutopilotCreateSchema.safeParse({ ...valid, scheduleMinute: 0 }).success).toBe(true);
    expect(AutopilotCreateSchema.safeParse({ ...valid, scheduleMinute: 59 }).success).toBe(true);
  });

  it('accepts a record omitting scheduleHour and scheduleMinute (both optional)', () => {
    const {
      scheduleHour: _h,
      scheduleMinute: _m,
      ...withoutTime
    } = {
      ...valid,
      scheduleHour: 9,
      scheduleMinute: 0,
    };
    expect(AutopilotCreateSchema.safeParse(withoutTime).success).toBe(true);
  });

  it('rejects scheduleHour: 24 (out of range)', () => {
    expect(AutopilotCreateSchema.safeParse({ ...valid, scheduleHour: 24 }).success).toBe(false);
  });

  it('rejects scheduleHour: -1 (below zero)', () => {
    expect(AutopilotCreateSchema.safeParse({ ...valid, scheduleHour: -1 }).success).toBe(false);
  });

  it('rejects scheduleMinute: 60 (out of range)', () => {
    expect(AutopilotCreateSchema.safeParse({ ...valid, scheduleMinute: 60 }).success).toBe(false);
  });

  it('rejects scheduleHour: 9.5 (enforces .int())', () => {
    // A schema that drops .int() would accept 9.5 — this confirms the constraint is present.
    expect(AutopilotCreateSchema.safeParse({ ...valid, scheduleHour: 9.5 }).success).toBe(false);
  });

  it('rejects scheduleMinute: -1 (below zero)', () => {
    expect(AutopilotCreateSchema.safeParse({ ...valid, scheduleMinute: -1 }).success).toBe(false);
  });
});

describe('ScrapeBoardsRequestSchema', () => {
  it('requires at least one valid board id', () => {
    expect(() =>
      ScrapeBoardsRequestSchema.parse({ boards: ['unknown_board'], query: 'test' })
    ).toThrow();
  });

  it('rejects an empty boards array', () => {
    expect(() => ScrapeBoardsRequestSchema.parse({ boards: [], query: 'test' })).toThrow();
  });

  it('rejects more entries than the catalog size', () => {
    // BOARD_IDS entries are unique, so exceeding the catalog size requires a
    // duplicate — the real dedup+truncate defense lives server-side in Rust;
    // this schema bound only guards against a grossly oversized payload.
    const tooMany = [...BOARD_IDS, BOARD_IDS[0]];
    expect(() => ScrapeBoardsRequestSchema.parse({ boards: tooMany, query: 'test' })).toThrow();
  });

  it('defaults amount to 25', () => {
    const result = ScrapeBoardsRequestSchema.parse({ boards: ['linkedin'], query: 'test' });
    expect(result.amount).toBe(25);
  });

  it('rejects amount above 100', () => {
    expect(() =>
      ScrapeBoardsRequestSchema.parse({ boards: ['linkedin'], query: 'test', amount: 101 })
    ).toThrow();
  });

  it('accepts every catalog board selected at once', () => {
    expect(() =>
      ScrapeBoardsRequestSchema.parse({ boards: [...BOARD_IDS], query: 'test' })
    ).not.toThrow();
  });

  it('rejects retired board id "indeed"', () => {
    expect(() => ScrapeBoardsRequestSchema.parse({ boards: ['indeed'], query: 'test' })).toThrow();
  });

  it('rejects retired board id "stepstone" even when mixed with a valid id', () => {
    expect(() =>
      ScrapeBoardsRequestSchema.parse({ boards: ['linkedin', 'stepstone'], query: 'test' })
    ).toThrow();
  });
});
