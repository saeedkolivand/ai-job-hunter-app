import { afterEach, describe, expect, it } from 'vitest';

import { readModelListCache, writeModelListCache } from './model-list-cache';

afterEach(() => {
  localStorage.clear();
});

describe('model-list-cache', () => {
  it('returns undefined when nothing has been cached for a provider', () => {
    expect(readModelListCache('openai')).toBeUndefined();
  });

  it('round-trips a written model list', () => {
    writeModelListCache('openai', undefined, [{ name: 'gpt-4o' }, { name: 'o1' }]);
    expect(readModelListCache('openai')).toEqual([{ name: 'gpt-4o' }, { name: 'o1' }]);
  });

  it('keys the cache by provider + base URL — a different base URL misses', () => {
    writeModelListCache('openai-compatible', 'http://localhost:1234/v1', [{ name: 'local-model' }]);
    expect(readModelListCache('openai-compatible', 'http://localhost:1234/v1')).toEqual([
      { name: 'local-model' },
    ]);
    expect(readModelListCache('openai-compatible', 'http://other:1234/v1')).toBeUndefined();
    expect(readModelListCache('openai-compatible')).toBeUndefined();
  });

  it('ignores a corrupt cache entry instead of throwing', () => {
    localStorage.setItem('ajh:model-list-cache:openai:', 'not json');
    expect(readModelListCache('openai')).toBeUndefined();
  });

  it('ignores a cache entry that is not a model list', () => {
    localStorage.setItem('ajh:model-list-cache:openai:', JSON.stringify({ not: 'a list' }));
    expect(readModelListCache('openai')).toBeUndefined();
  });

  it('overwrites a previous cached list for the same provider + base URL', () => {
    writeModelListCache('openai', undefined, [{ name: 'first' }]);
    writeModelListCache('openai', undefined, [{ name: 'second' }]);
    expect(readModelListCache('openai')).toEqual([{ name: 'second' }]);
  });
});
