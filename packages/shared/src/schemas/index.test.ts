import { describe, it, expect } from 'vitest';
import {
  AiGenerateRequestSchema,
  LocaleSchema,
  AutopilotCreateSchema,
  ScrapeBoardRequestSchema,
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
    target: { board: 'linkedin', query: 'react developer', pages: 2 },
    filter: { minMatchScore: 60 },
    action: 'save',
    schedule: 'daily',
    autoSubmit: false,
  };

  it('accepts a valid autopilot', () => {
    expect(() => AutopilotCreateSchema.parse(valid)).not.toThrow();
  });

  it('rejects empty name', () => {
    expect(() => AutopilotCreateSchema.parse({ ...valid, name: '' })).toThrow();
  });

  it('rejects invalid action', () => {
    expect(() => AutopilotCreateSchema.parse({ ...valid, action: 'unknown' })).toThrow();
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
});

describe('ScrapeBoardRequestSchema', () => {
  it('requires a supported board id', () => {
    expect(() =>
      ScrapeBoardRequestSchema.parse({ board: 'unknown_board', query: 'test' })
    ).toThrow();
  });

  it('defaults pages to 1', () => {
    const result = ScrapeBoardRequestSchema.parse({ board: 'linkedin', query: 'test' });
    expect(result.pages).toBe(1);
  });
});
