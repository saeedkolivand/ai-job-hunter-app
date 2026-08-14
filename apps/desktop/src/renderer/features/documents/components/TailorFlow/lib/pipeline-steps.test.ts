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
    // max-depth-only stages (never reached by the quality pipeline this
    // checklist renders) and a hypothetical future stage both fall here.
    expect(pipelineStepForStage('sections', 1)).toBe(1);
    expect(pipelineStepForStage('assemble', 2)).toBe(2);
    expect(pipelineStepForStage('llm_judge', 3)).toBe(3);
    expect(pipelineStepForStage('some_future_stage', 0)).toBe(0);
  });
});
