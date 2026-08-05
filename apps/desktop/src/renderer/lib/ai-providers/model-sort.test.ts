import { describe, expect, it } from 'vitest';

import type { ProviderModelInfo } from '@ajh/shared';

import { sortModelsNewestFirst } from './model-sort';

describe('sortModelsNewestFirst', () => {
  it('sorts entries with a createdAt newest-first', () => {
    const sorted = sortModelsNewestFirst([
      { name: 'a', createdAt: 100 },
      { name: 'b', createdAt: 300 },
      { name: 'c', createdAt: 200 },
    ]);
    expect(sorted.map((m) => m.name)).toEqual(['b', 'c', 'a']);
  });

  it('puts entries without createdAt after entries that have it', () => {
    const sorted = sortModelsNewestFirst([{ name: 'no-date' }, { name: 'dated', createdAt: 500 }]);
    expect(sorted.map((m) => m.name)).toEqual(['dated', 'no-date']);
  });

  it('leaves a list where every entry lacks createdAt (e.g. Gemini) in its original order', () => {
    const models: ProviderModelInfo[] = [{ name: 'first' }, { name: 'second' }, { name: 'third' }];
    expect(sortModelsNewestFirst(models).map((m) => m.name)).toEqual(['first', 'second', 'third']);
  });
});
