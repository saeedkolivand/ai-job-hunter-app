import { describe, expect, it } from 'vitest';

import {
  AutopilotTargetSchema,
  AutopilotUpdateSchema,
  ConversationSaveMessageSchema,
  CredentialSetSchema,
  DocumentImportRequestSchema,
  EmbedRequestSchema,
  HybridSearchRequestSchema,
  JobPreferencesSchema,
  MatchResumeRequestSchema,
  ResumeExtractTextSchema,
  ScrapeUrlRequestSchema,
} from './index';

describe('DocumentImportRequestSchema', () => {
  const bytes = new Uint8Array([1, 2, 3]);

  it('accepts a valid import', () => {
    expect(() => DocumentImportRequestSchema.parse({ name: 'resume.pdf', bytes })).not.toThrow();
  });

  it('rejects empty filename and oversized names', () => {
    expect(() => DocumentImportRequestSchema.parse({ name: '', bytes })).toThrow();
    expect(() => DocumentImportRequestSchema.parse({ name: 'a'.repeat(513), bytes })).toThrow();
  });

  it('rejects empty byte arrays', () => {
    expect(() =>
      DocumentImportRequestSchema.parse({ name: 'resume.pdf', bytes: new Uint8Array(0) })
    ).toThrow();
  });

  it('rejects files over 50 MB', () => {
    const big = new Uint8Array(50 * 1024 * 1024 + 1);
    expect(() => DocumentImportRequestSchema.parse({ name: 'big.pdf', bytes: big })).toThrow();
  });
});

describe('ScrapeUrlRequestSchema', () => {
  it('requires a valid URL', () => {
    expect(() => ScrapeUrlRequestSchema.parse({ url: 'https://example.com' })).not.toThrow();
    expect(() => ScrapeUrlRequestSchema.parse({ url: 'not-a-url' })).toThrow();
  });
});

describe('HybridSearchRequestSchema', () => {
  it('applies defaults for topK and semanticWeight', () => {
    const parsed = HybridSearchRequestSchema.parse({ query: 'react', collection: 'jobs' });
    expect(parsed.topK).toBe(20);
    expect(parsed.semanticWeight).toBeCloseTo(0.7);
  });

  it('rejects unknown collections and out-of-range weights', () => {
    expect(() => HybridSearchRequestSchema.parse({ query: 'x', collection: 'unknown' })).toThrow();
    expect(() =>
      HybridSearchRequestSchema.parse({ query: 'x', collection: 'jobs', semanticWeight: 1.5 })
    ).toThrow();
  });
});

describe('MatchResumeRequestSchema', () => {
  it('requires both ids', () => {
    expect(() => MatchResumeRequestSchema.parse({ resumeId: 'r1', jobId: 'j1' })).not.toThrow();
    expect(() => MatchResumeRequestSchema.parse({ resumeId: '', jobId: 'j1' })).toThrow();
  });
});

describe('CredentialSetSchema', () => {
  it('accepts supported boards', () => {
    expect(() =>
      CredentialSetSchema.parse({ boardId: 'linkedin', username: 'a', password: 'b' })
    ).not.toThrow();
  });

  it('rejects unsupported boards and overlong fields', () => {
    expect(() =>
      CredentialSetSchema.parse({ boardId: 'monster', username: 'a', password: 'b' })
    ).toThrow();
    expect(() =>
      CredentialSetSchema.parse({ boardId: 'xing', username: 'a'.repeat(255), password: 'b' })
    ).toThrow();
  });
});

describe('EmbedRequestSchema', () => {
  it('accepts text and optional model', () => {
    expect(() => EmbedRequestSchema.parse({ text: 'hello' })).not.toThrow();
    expect(() => EmbedRequestSchema.parse({ text: 'hello', model: 'nomic' })).not.toThrow();
  });

  it('rejects empty and oversized text', () => {
    expect(() => EmbedRequestSchema.parse({ text: '' })).toThrow();
    expect(() => EmbedRequestSchema.parse({ text: 'a'.repeat(200_001) })).toThrow();
  });
});

describe('ConversationSaveMessageSchema', () => {
  it('fills defaults for every field', () => {
    const parsed = ConversationSaveMessageSchema.parse({});
    expect(parsed.conversationId).toBe('default');
    expect(parsed.role).toBe('user');
    expect(parsed.content).toBe('');
  });
});

describe('ResumeExtractTextSchema', () => {
  it('rejects files over 25 MB', () => {
    const big = new Uint8Array(25 * 1024 * 1024 + 1);
    expect(() => ResumeExtractTextSchema.parse({ name: 'r.pdf', bytes: big })).toThrow();
  });

  it('accepts a small valid file', () => {
    expect(() =>
      ResumeExtractTextSchema.parse({ name: 'r.pdf', bytes: new Uint8Array([9]) })
    ).not.toThrow();
  });
});

describe('AutopilotTargetSchema', () => {
  it('defaults pages to 2', () => {
    const parsed = AutopilotTargetSchema.parse({ board: 'linkedin', query: 'dev' });
    expect(parsed.pages).toBe(2);
  });

  it('rejects pages above 10', () => {
    expect(() =>
      AutopilotTargetSchema.parse({ board: 'linkedin', query: 'dev', pages: 11 })
    ).toThrow();
  });
});

describe('AutopilotUpdateSchema', () => {
  it('allows a partial update with status', () => {
    expect(() => AutopilotUpdateSchema.parse({ status: 'paused' })).not.toThrow();
    expect(() => AutopilotUpdateSchema.parse({})).not.toThrow();
  });

  it('rejects an invalid status', () => {
    expect(() => AutopilotUpdateSchema.parse({ status: 'deleted' })).toThrow();
  });
});

describe('JobPreferencesSchema', () => {
  it('accepts a full preferences object', () => {
    expect(() =>
      JobPreferencesSchema.parse({
        location: 'Berlin',
        remote: 'hybrid',
        seniority: 'senior',
        techStack: [{ name: 'React', category: 'frontend' }],
      })
    ).not.toThrow();
  });

  it('accepts an empty object (all optional)', () => {
    expect(() => JobPreferencesSchema.parse({})).not.toThrow();
  });

  it('rejects a tech stack item missing a category', () => {
    expect(() => JobPreferencesSchema.parse({ techStack: [{ name: 'React' }] })).toThrow();
  });
});
