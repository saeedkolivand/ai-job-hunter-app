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

  it('does NOT conflate a genuine createdAt: 0 with "absent" — an undated entry still sorts after it', () => {
    // `createdAt ?? 0` would make these compare equal (both key to 0) and
    // rely on stable-sort to keep the undated entry wherever it started —
    // even AHEAD of the epoch-zero entry, which is wrong: epoch zero is a
    // real (if absurd) date and should outrank "no date at all" the same way
    // any other dated entry does.
    const models: ProviderModelInfo[] = [{ name: 'undated' }, { name: 'epoch-zero', createdAt: 0 }];
    expect(sortModelsNewestFirst(models).map((m) => m.name)).toEqual(['epoch-zero', 'undated']);
  });

  it('ranks a genuine createdAt: 0 correctly against a real positive timestamp', () => {
    const models: ProviderModelInfo[] = [
      { name: 'epoch-zero', createdAt: 0 },
      { name: 'newer', createdAt: 1_000 },
    ];
    expect(sortModelsNewestFirst(models).map((m) => m.name)).toEqual(['newer', 'epoch-zero']);
  });
});
