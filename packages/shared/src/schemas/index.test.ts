import { describe, expect, it } from 'vitest';

import {
  AgentRunRequestSchema,
  AiGenerateRequestSchema,
  AutopilotCreateSchema,
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
    provider: 'openai',
    model: 'gpt-4o',
  };

  it('accepts a valid request (baseUrl optional)', () => {
    expect(() => AgentRunRequestSchema.parse(valid)).not.toThrow();
    expect(() =>
      AgentRunRequestSchema.parse({ ...valid, baseUrl: 'http://localhost:11434' })
    ).not.toThrow();
  });

  it('requires resumeId, jobId, provider, and model (each non-empty)', () => {
    for (const key of ['resumeId', 'jobId', 'provider', 'model'] as const) {
      const { [key]: _omitted, ...without } = valid;
      expect(() => AgentRunRequestSchema.parse(without)).toThrow();
      expect(() => AgentRunRequestSchema.parse({ ...valid, [key]: '' })).toThrow();
    }
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

  it('rejects more than 6 boards', () => {
    expect(() =>
      ScrapeBoardsRequestSchema.parse({
        boards: ['linkedin', 'arbeitsagentur', 'greenhouse', 'lever', 'ashby', 'remotive', 'wwr'],
        query: 'test',
      })
    ).toThrow();
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

  it('accepts up to 6 boards', () => {
    expect(() =>
      ScrapeBoardsRequestSchema.parse({
        boards: ['linkedin', 'arbeitsagentur', 'greenhouse', 'lever', 'ashby', 'remotive'],
        query: 'test',
      })
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
