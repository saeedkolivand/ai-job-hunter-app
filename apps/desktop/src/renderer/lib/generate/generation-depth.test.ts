import { describe, expect, it } from 'vitest';

import { GENERATION_DEPTHS } from '@ajh/shared/schemas';

import {
  ALL_GENERATION_DEPTHS,
  type GenerationDepth,
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

  it('runs all three depths as of Phase 4 — max included', () => {
    // `resume_pipeline_run` routes `depth: "max"` to the section-wise pipeline
    // now (it used to reject it), so offering it is a true claim about what
    // would run. Pinned as a literal list rather than compared to
    // GENERATION_DEPTHS: "everything in the shared enum is runnable" is the
    // assertion that would silently start passing for a tier a FUTURE build
    // adds to the wire before this one can run it.
    expect(RUNNABLE_GENERATION_DEPTHS).toEqual(['fast', 'quality', 'max']);
    expect(isDepthRunnable('fast')).toBe(true);
    expect(isDepthRunnable('quality')).toBe(true);
    expect(isDepthRunnable('max')).toBe(true);
  });

  it('falls back to fast — the cheapest depth — for anything unrunnable', () => {
    // Never upward to a staged depth: a stored value naming a tier this build
    // doesn't have (a forward-migrated preference from a newer build) must not
    // buy someone a multi-call run they didn't ask for. No current tier takes
    // this path, so the cast is what keeps the guard reachable.
    expect(resolveRunnableDepth('not-a-depth' as GenerationDepth)).toBe('fast');
    expect(resolveRunnableDepth(undefined)).toBe('fast');
    expect(resolveRunnableDepth(null)).toBe('fast');
  });

  it('leaves a runnable depth alone', () => {
    expect(resolveRunnableDepth('quality')).toBe('quality');
    expect(resolveRunnableDepth('fast')).toBe('fast');
    expect(resolveRunnableDepth('max')).toBe('max');
  });
});
