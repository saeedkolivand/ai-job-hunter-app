import { describe, expect, it } from 'vitest';

import type { PipelineRunEvent } from '@ajh/shared/ipc';

import { usedKeywordFallback } from './weak-analysis';

const ev = (
  stage: string,
  phase: PipelineRunEvent['phase'],
  artifact: unknown
): PipelineRunEvent => ({
  seq: 1,
  ts: 1,
  stage,
  phase,
  artifact,
});

describe('usedKeywordFallback', () => {
  it('is true when analyze_job finished with keywordFallback', () => {
    expect(usedKeywordFallback([ev('analyze_job', 'finish', { keywordFallback: true })])).toBe(
      true
    );
  });
  it('is false for a clean analysis, other stages, null artifacts and no events', () => {
    expect(usedKeywordFallback([ev('analyze_job', 'finish', { keywordFallback: false })])).toBe(
      false
    );
    expect(usedKeywordFallback([ev('strategy', 'finish', { keywordFallback: true })])).toBe(false);
    expect(usedKeywordFallback([ev('analyze_job', 'finish', null)])).toBe(false);
    expect(usedKeywordFallback(undefined)).toBe(false);
  });
});
