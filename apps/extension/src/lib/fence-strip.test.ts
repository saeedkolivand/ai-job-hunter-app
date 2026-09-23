import { describe, expect, it } from 'vitest';

import { stripFenceWrapper } from './fence-strip';

// Mirrors the Rust `prompt_fence::test` suite for `strip_fence_wrapper`
// (the extension strips exactly what the desktop's `fenced` wrote — the
// two transliterations must match case-for-case).

describe('stripFenceWrapper', () => {
  it('reverses a real fenced value (the round-trip case the primitive exists for)', () => {
    const wrapped = `<job_posting>\nSenior Engineer\n</job_posting>`;
    expect(stripFenceWrapper('job_posting', wrapped)).toBe('Senior Engineer');
    expect(stripFenceWrapper('job_posting', '<job_posting>\nAcme Corp\n</job_posting>')).toBe(
      'Acme Corp'
    );
  });

  it('leaves an unwrapped value byte-for-byte unchanged (the common path)', () => {
    expect(stripFenceWrapper('job_posting', 'Senior Engineer')).toBe('Senior Engineer');
  });

  it('ignores a wrapper for a DIFFERENT tag — the match is on the exact tag name', () => {
    const wrapped = `<candidate_resume>\nsome resume text\n</candidate_resume>`;
    expect(stripFenceWrapper('job_posting', wrapped)).toBe(wrapped);
  });

  it('requires BOTH the exact open and close tag (a half-wrapped value survives)', () => {
    const half = `<job_posting>\nSenior Engineer`;
    expect(stripFenceWrapper('job_posting', half)).toBe(half);
    const half2 = `Senior Engineer\n</job_posting>`;
    expect(stripFenceWrapper('job_posting', half2)).toBe(half2);
  });

  it('leaves the empty-body overlap shape `<tag>\\n</tag>` unchanged (naive slice would corrupt it)', () => {
    const overlap = `<job_posting>\n</job_posting>`;
    expect(stripFenceWrapper('job_posting', overlap)).toBe(overlap);
  });

  it('strips an empty fenced body to the empty string', () => {
    expect(stripFenceWrapper('job_posting', `<job_posting>\n\n</job_posting>`)).toBe('');
  });

  it('passes a content that itself contains angle brackets through unchanged', () => {
    expect(stripFenceWrapper('job_posting', '<b>Senior</b> Rust Engineer')).toBe(
      '<b>Senior</b> Rust Engineer'
    );
  });

  it('leaves multi-line content with an em dash intact once unwrapped', () => {
    const wrapped = `<job_posting>\nSenior — Principal Engineer\nremote\n</job_posting>`;
    expect(stripFenceWrapper('job_posting', wrapped)).toBe('Senior — Principal Engineer\nremote');
  });
});
