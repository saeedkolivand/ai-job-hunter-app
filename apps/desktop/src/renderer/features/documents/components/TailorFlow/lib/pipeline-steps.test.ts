import { describe, expect, it } from 'vitest';

import { pipelineStepForStage } from './pipeline-steps';

describe('pipelineStepForStage', () => {
  it.each([
    ['analyze_job', 0],
    ['match_evidence', 0],
    ['strategy', 0],
    ['draft', 1],
    ['cover_letter', 1],
    ['validate', 2],
    ['repair', 2],
    ['humanize', 3],
  ] as const)('maps %s → step %i', (stage, expected) => {
    expect(pipelineStepForStage(stage, 0)).toBe(expected);
  });

  it('keeps the previous step for a stage name it does not recognise', () => {
    // A hypothetical future stage falls here rather than regressing the
    // checklist.
    expect(pipelineStepForStage('some_future_stage', 0)).toBe(0);
  });
});
