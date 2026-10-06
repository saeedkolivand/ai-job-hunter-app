import { describe, expect, it } from 'vitest';

import {
  MatchResumeRequestSchema,
  type ResumePipelineRunRequest,
  ResumePipelineRunSchema,
} from './index';

describe('MatchResumeRequestSchema', () => {
  it('requires both ids', () => {
    expect(() => MatchResumeRequestSchema.parse({ resumeId: 'r1', jobId: 'j1' })).not.toThrow();
    expect(() => MatchResumeRequestSchema.parse({ resumeId: '', jobId: 'j1' })).toThrow();
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
  // in `commands/resume_pipeline/tests/resolve_source.rs`) — whitespace counts as
  // empty on both refine rules, not just `''`.
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
