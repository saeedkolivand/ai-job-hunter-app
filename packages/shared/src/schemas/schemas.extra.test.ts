import { describe, expect, it } from 'vitest';

import {
  ApplicationTrackSchema,
  ApplicationUpdateSchema,
  AutopilotTargetSchema,
  AutopilotUpdateSchema,
  CredentialSetSchema,
  DocumentImportRequestSchema,
  EmbedRequestSchema,
  HelpSearchRequestSchema,
  JobPreferencesSchema,
  MatchResumeRequestSchema,
  PostingsHybridSearchRequestSchema,
  ResumeExtractTextSchema,
  type ResumePipelineRunRequest,
  ResumePipelineRunSchema,
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

  it('accepts the minimal shape and defaults locale, with no queryId at all', () => {
    // An agent-CLI caller sends neither field. `queryId` absent means "not
    // cancellable"; `locale` defaults so the drop list is still resolvable.
    const parsed = HelpSearchRequestSchema.parse(base);
    expect(parsed.queryId).toBeUndefined();
    expect(parsed.locale).toBe('en');
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

describe('ResumePipelineRunSchema', () => {
  it('parses the OLD id-only wire shape unchanged — zero behavior change', () => {
    const parsed = ResumePipelineRunSchema.parse({ resumeId: 'res-1', jobId: 'job-9' });
    expect(parsed.resumeId).toBe('res-1');
    expect(parsed.jobId).toBe('job-9');
    // The new fields default to the id-only path's no-op values.
    expect(parsed.resumeText).toBe('');
    expect(parsed.jobAdText).toBe('');
    expect(parsed.jobTitle).toBe('');
    expect(parsed.companyName).toBe('');
    expect(parsed.board).toBe('');
    expect(parsed.researchCompany).toBe(false);
  });

  it('accepts the text-only path — a pasted job ad, no ids', () => {
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a whole résumé',
        jobAdText: 'a whole job ad',
      })
    ).not.toThrow();
  });

  it('accepts a mix — an id for one side, text for the other', () => {
    expect(() =>
      ResumePipelineRunSchema.parse({ resumeId: 'res-1', jobAdText: 'a whole job ad' })
    ).not.toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({ resumeText: 'a whole résumé', jobId: 'job-9' })
    ).not.toThrow();
  });

  it('rejects an empty résumé side — neither resumeId nor resumeText', () => {
    expect(() => ResumePipelineRunSchema.parse({ jobId: 'job-9' })).toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({ resumeId: '', resumeText: '', jobId: 'job-9' })
    ).toThrow();
  });

  it('rejects an empty job side — neither jobId nor jobAdText', () => {
    expect(() => ResumePipelineRunSchema.parse({ resumeId: 'res-1' })).toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({ resumeId: 'res-1', jobId: '', jobAdText: '' })
    ).toThrow();
  });

  // Matches the Rust twin (`resume_source_is_none_when_both_are_empty_or_whitespace`
  // in `commands/resume_pipeline/test.rs`) — whitespace counts as empty on
  // both refine rules, not just `''`.
  it('rejects a whitespace-only résumé side and a whitespace-only job side', () => {
    expect(() =>
      ResumePipelineRunSchema.parse({ resumeId: '   ', resumeText: '\n\t', jobId: 'job-9' })
    ).toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({ resumeId: 'res-1', jobId: '   ', jobAdText: '\n\t' })
    ).toThrow();
  });

  it('caps the text-path free-text fields', () => {
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a'.repeat(200_001),
        jobAdText: 'a whole job ad',
      })
    ).toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a whole résumé',
        jobAdText: 'a'.repeat(200_001),
      })
    ).toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a whole résumé',
        jobAdText: 'a whole job ad',
        jobTitle: 'a'.repeat(513),
      })
    ).toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a whole résumé',
        jobAdText: 'a whole job ad',
        companyName: 'a'.repeat(513),
      })
    ).toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a whole résumé',
        jobAdText: 'a whole job ad',
        board: 'a'.repeat(65),
      })
    ).toThrow();
  });

  // The rejection side above proves the cap is enforced; this proves it is
  // enforced at the RIGHT boundary — an off-by-one that also rejected the
  // ceiling itself would still pass every `toThrow()` assertion above.
  it('accepts the text-path fields at exactly their caps (boundary — guards <= vs < off-by-one)', () => {
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a'.repeat(200_000),
        jobAdText: 'a whole job ad',
      })
    ).not.toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a whole résumé',
        jobAdText: 'a'.repeat(200_000),
      })
    ).not.toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a whole résumé',
        jobAdText: 'a whole job ad',
        jobTitle: 'a'.repeat(512),
      })
    ).not.toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a whole résumé',
        jobAdText: 'a whole job ad',
        companyName: 'a'.repeat(512),
      })
    ).not.toThrow();
    expect(() =>
      ResumePipelineRunSchema.parse({
        resumeText: 'a whole résumé',
        jobAdText: 'a whole job ad',
        board: 'a'.repeat(64),
      })
    ).not.toThrow();
  });
});

describe('ResumePipelineRunRequest (type-level)', () => {
  /**
   * `z.input`'s own output type leaves every field optional (each has a Zod
   * `.default(...)`), so `{}` — and a résumé-only or job-only object — used
   * to type-check despite being rejected by the schema's `.refine`s above
   * and by Rust at runtime. `@ts-expect-error` is checked by `tsc` (vitest's
   * esbuild does not type-check), and fails if any of these three ever
   * becomes assignable again — i.e. if the exported type is widened back
   * toward the bare `z.input` shape.
   */
  it('requires at least one résumé source AND at least one job source, at the type level', () => {
    // @ts-expect-error — neither a résumé source nor a job source.
    const neither: ResumePipelineRunRequest = {};
    // @ts-expect-error — a résumé source with no job source.
    const resumeOnly: ResumePipelineRunRequest = { resumeId: 'res-1' };
    // @ts-expect-error — a job source with no résumé source.
    const jobOnly: ResumePipelineRunRequest = { jobId: 'job-9' };

    // All four valid combinations type-check with no directive needed.
    const idBoth: ResumePipelineRunRequest = { resumeId: 'res-1', jobId: 'job-9' };
    const idThenText: ResumePipelineRunRequest = { resumeId: 'res-1', jobAdText: 'a job ad' };
    const textThenId: ResumePipelineRunRequest = { resumeText: 'a résumé', jobId: 'job-9' };
    const textBoth: ResumePipelineRunRequest = { resumeText: 'a résumé', jobAdText: 'a job ad' };

    expect([neither, resumeOnly, jobOnly, idBoth, idThenText, textThenId, textBoth]).toHaveLength(
      7
    );
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
    const parsed = AutopilotTargetSchema.parse({ boards: ['linkedin'], query: 'dev' });
    expect(parsed.pages).toBe(2);
  });

  it('rejects pages above 10', () => {
    expect(() =>
      AutopilotTargetSchema.parse({ boards: ['linkedin'], query: 'dev', pages: 11 })
    ).toThrow();
  });

  it('rejects an empty boards array', () => {
    expect(() => AutopilotTargetSchema.parse({ boards: [], query: 'dev' })).toThrow();
  });

  it('accepts more than 6 boards (catalog has grown past the old cap)', () => {
    expect(() =>
      AutopilotTargetSchema.parse({
        boards: [
          'linkedin',
          'arbeitsagentur',
          'remoteok',
          'greenhouse',
          'lever',
          'ashby',
          'remotive',
        ],
        query: 'dev',
      })
    ).not.toThrow();
  });

  it('rejects a grossly oversized boards array (sanity bound, not the real cap)', () => {
    // The real dedup+truncate defense is server-side (Rust registry cap); this
    // schema-level bound only guards against a corrupt/hostile payload.
    const tooMany = Array.from({ length: 65 }, (_, i) => `board_${i}`);
    expect(() => AutopilotTargetSchema.parse({ boards: tooMany, query: 'dev' })).toThrow();
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

  it('accepts a 2-letter countryCode', () => {
    expect(() =>
      JobPreferencesSchema.parse({ location: 'Berlin', countryCode: 'de' })
    ).not.toThrow();
  });

  it('rejects a malformed countryCode', () => {
    expect(() => JobPreferencesSchema.parse({ countryCode: 'deu' })).toThrow();
    expect(() => JobPreferencesSchema.parse({ countryCode: '1a' })).toThrow();
  });

  it('accepts a salaryExpectation string', () => {
    expect(() => JobPreferencesSchema.parse({ salaryExpectation: '€75,000' })).not.toThrow();
  });

  it('accepts an object with no salaryExpectation (optional, additive)', () => {
    const parsed = JobPreferencesSchema.parse({ location: 'Berlin' });
    expect(parsed.salaryExpectation).toBeUndefined();
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

describe('ApplicationTrackSchema — jobDescription carried from a posting', () => {
  it('accepts a track request carrying a jobDescription', () => {
    expect(() =>
      ApplicationTrackSchema.parse({
        jobUrl: 'https://example.com/job/1',
        board: 'aggregator',
        company: 'Acme',
        title: 'Engineer',
        jobDescription: 'Build things.',
      })
    ).not.toThrow();
  });

  it('accepts jobDescription absent (field is optional)', () => {
    expect(() =>
      ApplicationTrackSchema.parse({ jobUrl: 'https://example.com/job/1' })
    ).not.toThrow();
  });

  it('rejects a jobDescription that exceeds 200 000 bytes', () => {
    const overLimit = 'a'.repeat(200_001);
    expect(() => ApplicationTrackSchema.parse({ jobDescription: overLimit })).toThrow(
      /200000 bytes/
    );
  });

  it('enforces a BYTE ceiling, not a character ceiling (multi-byte UTF-8)', () => {
    // '€' encodes as 3 bytes in UTF-8. 66 667 '€' chars = 200 001 bytes but
    // only 66 667 chars — under the naive char limit but over the byte limit.
    const euroCount = 66_667;
    const overLimitByBytes = '€'.repeat(euroCount);
    expect(new TextEncoder().encode(overLimitByBytes).length).toBeGreaterThan(200_000);
    expect(() => ApplicationTrackSchema.parse({ jobDescription: overLimitByBytes })).toThrow(
      /200000 bytes/
    );
  });

  it('accepts a multi-byte string that stays under 200 000 bytes', () => {
    // 66 666 '€' = 199 998 bytes — just under the ceiling.
    const justUnder = '€'.repeat(66_666);
    expect(new TextEncoder().encode(justUnder).length).toBeLessThanOrEqual(200_000);
    expect(() => ApplicationTrackSchema.parse({ jobDescription: justUnder })).not.toThrow();
  });
});
