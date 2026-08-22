import { describe, expect, it } from 'vitest';

import { matchesWorkTypeFilter } from './work-type-filter';

describe('matchesWorkTypeFilter', () => {
  it('keeps every posting when nothing is requested (empty set = no filter)', () => {
    expect(matchesWorkTypeFilter({ workType: 'remote' }, [])).toBe(true);
    expect(matchesWorkTypeFilter({ workType: undefined }, [])).toBe(true);
  });

  it('keeps an undeclared posting even when a filter is active — the policy this pins', () => {
    // Mutation check: flipping this to `false` is exactly the false-negative
    // the backend's keep-unknowns policy exists to prevent (see the Lever
    // 87%-unspecified measurement in .claude/scratch/work-type-filter.md).
    expect(matchesWorkTypeFilter({ workType: undefined }, ['remote'])).toBe(true);
  });

  it('keeps a declared posting that matches the requested set', () => {
    expect(matchesWorkTypeFilter({ workType: 'hybrid' }, ['remote', 'hybrid'])).toBe(true);
  });

  it('drops a declared posting that does not match the requested set', () => {
    expect(matchesWorkTypeFilter({ workType: 'on-site' }, ['remote'])).toBe(false);
  });
});
