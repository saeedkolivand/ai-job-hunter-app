import { describe, expect, it } from 'vitest';

import { MATCH_LEVELS, scoreToLevel } from './match-level';

describe('scoreToLevel', () => {
  it.each([
    [0, 'low'],
    [29, 'low'],
    [39, 'low'],
    [40, 'medium'],
    [50, 'medium'],
    [64, 'medium'],
    [65, 'high'],
    [70, 'high'],
    [100, 'high'],
  ])('maps score %i to "%s"', (score, level) => {
    expect(scoreToLevel(score)).toBe(level);
  });

  it('each canonical level value resolves back to its own level (round-trip)', () => {
    for (const { id, value } of MATCH_LEVELS) {
      expect(scoreToLevel(value)).toBe(id);
    }
  });
});
