import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { useNeedsResearchKey } from './use-needs-research-key';

// Active provider + Ollama web-search key — controllable so each case exercises
// a different combination.
let mockProvider = 'openai';
let mockHasOllamaKey = true;
vi.mock('@/components/ui/ModelSelector', () => ({ useSelectedProvider: () => mockProvider }));
vi.mock('@/services', () => ({ useHasProviderKey: () => ({ data: { has: mockHasOllamaKey } }) }));

describe('useNeedsResearchKey', () => {
  it('flags true for an Ollama provider missing the web-search key', () => {
    mockProvider = 'ollama';
    mockHasOllamaKey = false;
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(true);
  });

  it('flags true for ollama-cloud missing the key too (same family)', () => {
    mockProvider = 'ollama-cloud';
    mockHasOllamaKey = false;
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(true);
  });

  it('is false once the Ollama key is present', () => {
    mockProvider = 'ollama';
    mockHasOllamaKey = true;
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(false);
  });

  it('is false for a non-Ollama provider, key or not', () => {
    mockProvider = 'openai';
    mockHasOllamaKey = false;
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(false);
  });
});
