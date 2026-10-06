import { describe, expect, it } from 'vitest';

import {
  CredentialSetSchema,
  DocumentImportRequestSchema,
  EmbedRequestSchema,
  HelpSearchRequestSchema,
  PostingsHybridSearchRequestSchema,
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

describe('PostingsHybridSearchRequestSchema', () => {
  const base = { queryId: 'search-q1', query: 'react developer' };

  it('accepts the minimal shape (no eligibleIds, no limit)', () => {
    expect(() => PostingsHybridSearchRequestSchema.parse(base)).not.toThrow();
  });

  it('requires a non-empty query and queryId', () => {
    expect(() => PostingsHybridSearchRequestSchema.parse({ ...base, query: '  ' })).toThrow();
    expect(() => PostingsHybridSearchRequestSchema.parse({ ...base, queryId: '' })).toThrow();
  });

  it('rejects a queryId that does not carry the required "search-" prefix', () => {
    // The Rust-side collision the prefix closes (`jobs::cancel::CancelRegistry`):
    // an unprefixed id could otherwise NAME a live `job-{uuid}`/`run-{uuid}` run.
    expect(() =>
      PostingsHybridSearchRequestSchema.parse({ ...base, queryId: '3f5d9b6a-uuid-with-no-prefix' })
    ).toThrow();
    expect(() =>
      PostingsHybridSearchRequestSchema.parse({ ...base, queryId: 'job-not-a-search' })
    ).toThrow();
  });

  it('rejects a query over the length cap', () => {
    expect(() =>
      PostingsHybridSearchRequestSchema.parse({ ...base, query: 'x'.repeat(201) })
    ).toThrow();
  });

  it('accepts an eligibleIds allowlist up to the cap and rejects past it', () => {
    expect(() =>
      PostingsHybridSearchRequestSchema.parse({ ...base, eligibleIds: ['a', 'b'] })
    ).not.toThrow();
    expect(() =>
      PostingsHybridSearchRequestSchema.parse({
        ...base,
        eligibleIds: Array.from({ length: 2001 }, (_, i) => `id-${i}`),
      })
    ).toThrow();
  });

  it('rejects a limit outside 1-50', () => {
    expect(() => PostingsHybridSearchRequestSchema.parse({ ...base, limit: 0 })).toThrow();
    expect(() => PostingsHybridSearchRequestSchema.parse({ ...base, limit: 51 })).toThrow();
    expect(() => PostingsHybridSearchRequestSchema.parse({ ...base, limit: 20 })).not.toThrow();
  });
});

describe('HelpSearchRequestSchema', () => {
  const base = {
    query: 'how do I export a resume?',
    entries: [{ id: 'aiGenerateQuestions.exportDoc', title: 'Export', body: 'Press Export.' }],
  };

  it('accepts the minimal shape with neither queryId nor locale', () => {
    // An agent-CLI caller sends neither field. `queryId` absent means "not
    // cancellable"; `locale` absent means "drop no function words" — NOT
    // English. Defaulting it here would have filtered a French or Japanese
    // corpus through an English drop list without anyone asking for it
    // (`commands::help::stopwords::stopwords_for_locale`).
    const parsed = HelpSearchRequestSchema.parse(base);
    expect(parsed.queryId).toBeUndefined();
    expect(parsed.locale).toBeUndefined();
  });

  it('still accepts an explicit locale', () => {
    expect(HelpSearchRequestSchema.parse({ ...base, locale: 'de-AT' }).locale).toBe('de-AT');
    // The caps stay: a 1-char tag is not a BCP-47 primary subtag, and an
    // unbounded string must never reach the Rust normaliser.
    expect(() => HelpSearchRequestSchema.parse({ ...base, locale: 'd' })).toThrow();
    expect(() => HelpSearchRequestSchema.parse({ ...base, locale: 'e'.repeat(17) })).toThrow();
  });

  it('accepts a queryId carrying the required "help-" prefix', () => {
    expect(() =>
      HelpSearchRequestSchema.parse({
        ...base,
        queryId: 'help-3f5d9b6a-1c2d-4e5f-8a9b-0c1d2e3f4a5b',
      })
    ).not.toThrow();
  });

  it('rejects a queryId that does not carry the required "help-" prefix', () => {
    // The Rust-side collision the prefix closes (`jobs::cancel::CancelRegistry`
    // is last-writer-wins): an unprefixed id could NAME a live
    // `job-{uuid}`/`run-{uuid}` run, and the postings search's own `search-`
    // space must stay disjoint from this one too.
    expect(() =>
      HelpSearchRequestSchema.parse({ ...base, queryId: '3f5d9b6a-uuid-with-no-prefix' })
    ).toThrow();
    expect(() => HelpSearchRequestSchema.parse({ ...base, queryId: 'job-not-a-help' })).toThrow();
    expect(() =>
      HelpSearchRequestSchema.parse({ ...base, queryId: 'search-a-postings-query' })
    ).toThrow();
    expect(() => HelpSearchRequestSchema.parse({ ...base, queryId: '' })).toThrow();
  });

  it('rejects a queryId past the 64-char cap', () => {
    expect(() =>
      HelpSearchRequestSchema.parse({ ...base, queryId: `help-${'x'.repeat(59)}` })
    ).not.toThrow();
    expect(() =>
      HelpSearchRequestSchema.parse({ ...base, queryId: `help-${'x'.repeat(60)}` })
    ).toThrow();
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
