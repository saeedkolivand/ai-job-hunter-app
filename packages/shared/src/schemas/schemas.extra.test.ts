import { describe, expect, it } from 'vitest';

import {
  ApplicationUpdateSchema,
  AutopilotTargetSchema,
  AutopilotUpdateSchema,
  CredentialSetSchema,
  DocumentImportRequestSchema,
  EmbedRequestSchema,
  JobPreferencesSchema,
  MatchResumeBatchRequestSchema,
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

  it('accepts text of exactly 200 000 bytes (boundary — guards <= vs < off-by-one)', () => {
    // 'a' is one byte in UTF-8, so this string is exactly at the allowed ceiling.
    expect(() => EmbedRequestSchema.parse({ text: 'a'.repeat(200_000) })).not.toThrow();
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

describe('ApplicationUpdateSchema — jobDescription byte-level refine', () => {
  it('accepts a valid jobDescription well under 200 000 bytes', () => {
    expect(() =>
      ApplicationUpdateSchema.parse({ id: 'app1', jobDescription: 'A short description.' })
    ).not.toThrow();
  });

  it('accepts jobDescription absent (field is optional)', () => {
    expect(() => ApplicationUpdateSchema.parse({ id: 'app1' })).not.toThrow();
  });

  it('rejects a jobDescription that exceeds 200 000 bytes', () => {
    // Each 'a' is one byte — 200 001 bytes pushes past the ceiling.
    const overLimit = 'a'.repeat(200_001);
    expect(() => ApplicationUpdateSchema.parse({ id: 'app1', jobDescription: overLimit })).toThrow(
      /200000 bytes/
    );
  });

  it('enforces a BYTE ceiling, not a character ceiling (multi-byte UTF-8)', () => {
    // '€' encodes as 3 bytes in UTF-8. 66 667 '€' chars = 200 001 bytes but
    // only 66 667 chars — under the naive char limit but over the byte limit.
    const euroCount = 66_667;
    const overLimitByBytes = '€'.repeat(euroCount);
    // Verify the fixture actually exceeds 200 000 bytes.
    expect(new TextEncoder().encode(overLimitByBytes).length).toBeGreaterThan(200_000);
    // And that the schema rejects it.
    expect(() =>
      ApplicationUpdateSchema.parse({ id: 'app1', jobDescription: overLimitByBytes })
    ).toThrow(/200000 bytes/);
  });

  it('accepts a multi-byte string that stays under 200 000 bytes', () => {
    // 66 666 '€' = 199 998 bytes — just under the ceiling.
    const justUnder = '€'.repeat(66_666);
    expect(new TextEncoder().encode(justUnder).length).toBeLessThanOrEqual(200_000);
    expect(() =>
      ApplicationUpdateSchema.parse({ id: 'app1', jobDescription: justUnder })
    ).not.toThrow();
  });
});

describe('MatchResumeBatchRequestSchema — jobIds boundary', () => {
  it('accepts exactly 1 000 job IDs (the max)', () => {
    const jobIds = Array.from({ length: 1000 }, (_, i) => `job-${String(i)}`);
    expect(() => MatchResumeBatchRequestSchema.parse({ resumeId: 'r1', jobIds })).not.toThrow();
  });

  it('rejects 1 001 job IDs (one over the max)', () => {
    const jobIds = Array.from({ length: 1001 }, (_, i) => `job-${String(i)}`);
    expect(() => MatchResumeBatchRequestSchema.parse({ resumeId: 'r1', jobIds })).toThrow();
  });

  it('accepts an empty jobIds array (no min constraint)', () => {
    expect(() => MatchResumeBatchRequestSchema.parse({ resumeId: 'r1', jobIds: [] })).not.toThrow();
  });
});
