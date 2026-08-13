/**
 * Real (unmocked) coverage for `useCanUseAI` itself.
 *
 * Every existing CONSUMER test mocks this hook wholesale (see
 * `ApplyByEmailTab.test.tsx`: `vi.mock('@/components/ui/ModelSelector', ...)`),
 * so its real branches — especially local-server, which never read health at
 * all before this fix — had zero coverage anywhere in the repo. This exercises
 * the real hook through a real (mock-backed) `AppClient` + `QueryClient`.
 */
import { describe, expect, it } from 'vitest';
import { waitFor } from '@testing-library/react';

import type { RuntimeHealth } from '@ajh/shared';

import { createMockClient } from '@/lib/mock-client';
import { renderHookWithClient } from '@/test-support';

import { useCanUseAI } from './index';

const readyHealth = (overrides: Partial<RuntimeHealth> = {}): RuntimeHealth => ({
  ai: { ready: true, model: 'llama3.2' },
  cliAgents: { 'claude-code': { detected: true } },
  data: { ready: true, sqlite: true, vector: true },
  workers: { active: 0, idle: 0, max: 4 },
  ...overrides,
});

describe('useCanUseAI — local-server (Ollama)', () => {
  it('configured model + daemon NOT ready → blocked with the "start Ollama" reason', async () => {
    // Pins the bug this suite exists for: a configured model used to be
    // treated as "ready" with no regard for whether the daemon was actually
    // reachable.
    const client = createMockClient({
      ai: {
        activeConfig: async () => ({
          activeProvider: 'ollama',
          providers: { ollama: { model: 'llama3.2' } },
        }),
      },
      system: { health: async () => readyHealth({ ai: { ready: false } }) },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() => expect(result.current).toEqual({ canUse: false, reason: 'startOllama' }));
  });

  it('configured model + daemon ready → usable', async () => {
    const client = createMockClient({
      ai: {
        activeConfig: async () => ({
          activeProvider: 'ollama',
          providers: { ollama: { model: 'llama3.2' } },
        }),
      },
      system: { health: async () => readyHealth() },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() => expect(result.current).toEqual({ canUse: true }));
  });

  it('no model configured → blocked with "select a model", never reaches the health probe', async () => {
    const client = createMockClient({
      ai: { activeConfig: async () => ({ activeProvider: 'ollama', providers: {} }) },
      // Daemon reported not-ready too — proves selectModel wins first, not a
      // side effect of the health probe being skipped/absent.
      system: { health: async () => readyHealth({ ai: { ready: false } }) },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() => expect(result.current).toEqual({ canUse: false, reason: 'selectModel' }));
  });

  it('model configured but the health probe is still in flight → "checking" (no reason), never a premature "start Ollama"', async () => {
    const client = createMockClient({
      ai: {
        activeConfig: async () => ({
          activeProvider: 'ollama',
          providers: { ollama: { model: 'llama3.2' } },
        }),
      },
      // Never resolves — the health query stays pending for the life of the
      // test, isolating the exact window `useActiveConfig` has already
      // settled but `useSystemHealth` has not.
      system: { health: () => new Promise<RuntimeHealth>(() => {}) },
    });
    const { result, queryClient } = renderHookWithClient(() => useCanUseAI(), { client });
    // `{ canUse: false }` (no reason) is ALSO the very first synchronous
    // render's value (useActiveConfig's own cold-boot branch) — asserting it
    // right away would pass trivially without ever exercising the health
    // gate. Require `useActiveConfig` to have actually SETTLED in the same
    // assertion, so this only passes once we're provably past that branch.
    await waitFor(() => {
      expect(queryClient.getQueryState(['ai', 'activeConfig'])?.status).toBe('success');
      expect(result.current).toEqual({ canUse: false });
    });
  });
});

describe('useCanUseAI — cli-agent', () => {
  it('binary detected → usable', async () => {
    const client = createMockClient({
      ai: { activeConfig: async () => ({ activeProvider: 'claude-code', providers: {} }) },
      system: {
        health: async () => readyHealth({ cliAgents: { 'claude-code': { detected: true } } }),
      },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() => expect(result.current).toEqual({ canUse: true }));
  });

  it('binary not detected → blocked with "install the CLI"', async () => {
    const client = createMockClient({
      ai: { activeConfig: async () => ({ activeProvider: 'claude-code', providers: {} }) },
      system: {
        health: async () => readyHealth({ cliAgents: { 'claude-code': { detected: false } } }),
      },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() => expect(result.current).toEqual({ canUse: false, reason: 'installCli' }));
  });
});

describe('useCanUseAI — cloud', () => {
  it('no stored key → blocked with "add an API key"', async () => {
    const client = createMockClient({
      ai: {
        activeConfig: async () => ({
          activeProvider: 'openai',
          providers: { openai: { model: 'gpt-4o' } },
        }),
        hasProviderKey: async () => ({ has: false }),
      },
      system: { health: async () => readyHealth() },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() => expect(result.current).toEqual({ canUse: false, reason: 'addApiKey' }));
  });

  it('key stored but no model chosen → blocked with "select a model"', async () => {
    const client = createMockClient({
      ai: {
        activeConfig: async () => ({ activeProvider: 'openai', providers: {} }),
        hasProviderKey: async () => ({ has: true }),
      },
      system: { health: async () => readyHealth() },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() => expect(result.current).toEqual({ canUse: false, reason: 'selectModel' }));
  });

  it('key stored + model chosen → usable', async () => {
    const client = createMockClient({
      ai: {
        activeConfig: async () => ({
          activeProvider: 'openai',
          providers: { openai: { model: 'gpt-4o' } },
        }),
        hasProviderKey: async () => ({ has: true }),
      },
      system: { health: async () => readyHealth() },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() => expect(result.current).toEqual({ canUse: true }));
  });
});
