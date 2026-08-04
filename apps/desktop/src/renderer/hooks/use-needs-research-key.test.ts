import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { useHasProviderKey } from '@/services';

import { useNeedsResearchKey } from './use-needs-research-key';

// Two independently-controllable queries — the active-config query (provider +
// its OWN pending window) and the ollama-cloud key query. Each mock derives
// `data`/`isPending` the way real React Query does (data is `undefined` while
// pending), so every combination from the review is reachable from here: both
// settled, either query still pending, and the provider/key value itself.
let mockConfigPending = false;
let mockActiveProvider = 'openai';
let mockKeyPending = false;
let mockHasOllamaKey = true;

vi.mock('@/services', () => ({
  useActiveConfig: vi.fn(() => ({
    data: mockConfigPending ? undefined : { activeProvider: mockActiveProvider },
    isPending: mockConfigPending,
  })),
  useHasProviderKey: vi.fn((_provider: string) => ({
    data: mockKeyPending ? undefined : { has: mockHasOllamaKey },
    isPending: mockKeyPending,
  })),
}));

describe('useNeedsResearchKey', () => {
  beforeEach(() => {
    mockConfigPending = false;
    mockActiveProvider = 'openai';
    mockKeyPending = false;
    mockHasOllamaKey = true;
    vi.mocked(useHasProviderKey).mockClear();
  });

  it('reads the ollama-cloud credential slot, not a different one', () => {
    renderHook(() => useNeedsResearchKey());
    expect(useHasProviderKey).toHaveBeenCalledWith('ollama-cloud');
  });

  it('flags true for an Ollama provider missing the web-search key', () => {
    mockActiveProvider = 'ollama';
    mockHasOllamaKey = false;
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(true);
  });

  it('flags true for ollama-cloud missing the key too (same family)', () => {
    mockActiveProvider = 'ollama-cloud';
    mockHasOllamaKey = false;
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(true);
  });

  it('is false once the Ollama key is present', () => {
    mockActiveProvider = 'ollama';
    mockHasOllamaKey = true;
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(false);
  });

  it('is false for a non-Ollama provider, key or not', () => {
    mockActiveProvider = 'openai';
    mockHasOllamaKey = false;
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(false);
  });

  it('suppresses the hint while the active-config query is pending (cold boot defaults to ollama)', () => {
    mockConfigPending = true;
    mockHasOllamaKey = false;
    // If the pending default ('ollama') were trusted, this would wrongly read
    // true for an OpenAI/Anthropic/CLI-agent user whose config hasn't landed.
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(false);
  });

  it('suppresses the hint while only the key query is pending, even for a resolved Ollama provider', () => {
    mockActiveProvider = 'ollama';
    mockKeyPending = true;
    // The key IS present — but its query hasn't landed, so `data` reads
    // undefined exactly like the real hook mid-flight. A user who already
    // added the key must not see the hint flash on first paint.
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(false);
  });

  it('settles to true once both queries land on an Ollama provider still missing the key', () => {
    mockActiveProvider = 'ollama';
    mockHasOllamaKey = false;
    expect(renderHook(() => useNeedsResearchKey()).result.current).toBe(true);
  });
});
