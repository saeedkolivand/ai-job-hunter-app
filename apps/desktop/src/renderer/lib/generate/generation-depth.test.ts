import { describe, expect, it } from 'vitest';

import { GENERATION_DEPTHS } from '@ajh/shared/schemas';

import {
  ALL_GENERATION_DEPTHS,
  isDepthRunnable,
  resolveRunnableDepth,
  RUNNABLE_GENERATION_DEPTHS,
} from './generation-depth';

describe('generation depth', () => {
  it('describes every depth in the shared vocabulary', () => {
    // The info popover walks this list; a depth added to the shared enum must
    // show up there rather than silently going undocumented.
    expect(ALL_GENERATION_DEPTHS).toEqual(GENERATION_DEPTHS);
  });

  it('runs fast and quality, and NOT max', () => {
    // `resume_pipeline_run` rejects `depth: "max"` with a validation error —
    // offering it as selectable would be a lie about what would run.
    expect(RUNNABLE_GENERATION_DEPTHS).toEqual(['fast', 'quality']);
    expect(isDepthRunnable('fast')).toBe(true);
    expect(isDepthRunnable('quality')).toBe(true);
    expect(isDepthRunnable('max')).toBe(false);
  });

  it('falls back to fast — the cheapest depth — for anything unrunnable', () => {
    // Never to `quality`: silently upgrading someone into four provider calls
    // plus two repair rounds because a stored value named a tier that does not
    // exist yet is the one wrong answer here.
    expect(resolveRunnableDepth('max')).toBe('fast');
    expect(resolveRunnableDepth(undefined)).toBe('fast');
    expect(resolveRunnableDepth(null)).toBe('fast');
  });

  it('leaves a runnable depth alone', () => {
    expect(resolveRunnableDepth('quality')).toBe('quality');
    expect(resolveRunnableDepth('fast')).toBe('fast');
  });
});
