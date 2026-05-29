import { describe, expect, it } from 'vitest';

import { getRecommended, MODEL_RECS } from './ai-models';

describe('MODEL_RECS', () => {
  it('lists models in ascending resource order', () => {
    expect(MODEL_RECS).toHaveLength(4);
    for (let i = 1; i < MODEL_RECS.length; i++) {
      expect(MODEL_RECS[i]?.minRamGb ?? 0).toBeGreaterThanOrEqual(MODEL_RECS[i - 1]?.minRamGb ?? 0);
    }
  });

  it('gives every model the required fields', () => {
    for (const m of MODEL_RECS) {
      expect(m.name).toBeTruthy();
      expect(m.label).toBeTruthy();
      expect(m.sizeGb).toBeGreaterThan(0);
      expect(m.estimatedRamGb).toBeGreaterThan(0);
    }
  });
});

describe('getRecommended', () => {
  it('recommends the 8B model for 12GB+ RAM', () => {
    expect(getRecommended(12).name).toBe('llama3.1:8b');
    expect(getRecommended(64).name).toBe('llama3.1:8b');
  });

  it('recommends Mistral for 10–11GB RAM', () => {
    expect(getRecommended(10).name).toBe('mistral');
    expect(getRecommended(11).name).toBe('mistral');
  });

  it('recommends the 3B model for 6–9GB RAM', () => {
    expect(getRecommended(6).name).toBe('llama3.2');
    expect(getRecommended(9).name).toBe('llama3.2');
  });

  it('recommends the 1B model for low RAM', () => {
    expect(getRecommended(4).name).toBe('llama3.2:1b');
    expect(getRecommended(0).name).toBe('llama3.2:1b');
  });
});
