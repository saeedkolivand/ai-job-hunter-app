import { describe, expect, it } from 'vitest';

import { MODES } from '../index';

describe('MODES', () => {
  it('defines tone instructions for every mode', () => {
    for (const mode of Object.keys(MODES) as (keyof typeof MODES)[]) {
      expect(MODES[mode].label).toBeTruthy();
      expect(MODES[mode].toneInstruction.length).toBeGreaterThan(10);
    }
  });
});
