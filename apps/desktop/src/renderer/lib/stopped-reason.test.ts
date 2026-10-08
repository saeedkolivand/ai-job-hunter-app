import { describe, expect, it } from 'vitest';

import { stoppedSuffix } from './stopped-reason';

describe('stoppedSuffix', () => {
  it('maps the persisted provider-failure reason to its own label (#1393)', () => {
    expect(stoppedSuffix('provider_error')).toBe('providerError');
  });

  it('maps the output-limit cutoff to its own label', () => {
    expect(stoppedSuffix('output_limit')).toBe('outputLimit');
  });

  it('never turns a missing reason into a success label', () => {
    expect(stoppedSuffix(null)).toBeNull();
    expect(stoppedSuffix('brand_new')).toBe('stopped');
  });
});
