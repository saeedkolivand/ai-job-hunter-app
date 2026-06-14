/**
 * query-client — key factory unit tests.
 *
 * Focused on the Phase K addition: keys.match.batch must sort jobIds so the
 * cache key is stable regardless of the order the caller supplies them.
 */
import { describe, expect, it } from 'vitest';

import { keys } from './query-client';

describe('keys.match.batch — order-independence', () => {
  it('produces the same key when jobIds are supplied in different orders', () => {
    const forward = keys.match.batch('resume-1', ['b', 'a'], false);
    const reversed = keys.match.batch('resume-1', ['a', 'b'], false);
    expect(forward).toEqual(reversed);
  });

  it('is stable for a single jobId (no-op sort)', () => {
    expect(keys.match.batch('r', ['only'], true)).toEqual(keys.match.batch('r', ['only'], true));
  });

  it('differs when resumeId changes', () => {
    const a = keys.match.batch('resume-a', ['job-1', 'job-2'], false);
    const b = keys.match.batch('resume-b', ['job-1', 'job-2'], false);
    expect(a).not.toEqual(b);
  });

  it('differs when semantic flag changes', () => {
    const on = keys.match.batch('r', ['a', 'b'], true);
    const off = keys.match.batch('r', ['a', 'b'], false);
    expect(on).not.toEqual(off);
  });

  it('differs when the jobId set changes', () => {
    const one = keys.match.batch('r', ['a', 'b'], false);
    const two = keys.match.batch('r', ['a', 'c'], false);
    expect(one).not.toEqual(two);
  });

  it('uses the match-batch namespace, not the single-score namespace', () => {
    const batchKey = keys.match.batch('r', ['j'], false);
    const singleKey = keys.match.score('r', 'j');
    expect(batchKey[0]).toBe('match-batch');
    expect(singleKey[0]).toBe('match');
  });
});
