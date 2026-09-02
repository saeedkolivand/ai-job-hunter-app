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

  it('model configured but the health probe REJECTS → the distinct "health unavailable" reason, not a silent "checking" forever', async () => {
    // Before this fix: `health` stays `undefined` on a rejected query exactly
    // like it does while pending, so this fell into the same branch as "still
    // checking" — permanently, since a rejected query with `retry: false`
    // never resolves on its own. A consumer rendering `AiSetupHint` with no
    // `reason` then fell back to its own default (`aiSetup.addApiKey`) — a
    // flatly wrong hint for an Ollama user mid network hiccup.
    const client = createMockClient({
      ai: {
        activeConfig: async () => ({
          activeProvider: 'ollama',
          providers: { ollama: { model: 'llama3.2' } },
        }),
      },
      system: { health: () => Promise.reject(new Error('health probe unreachable')) },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() =>
      expect(result.current).toEqual({ canUse: false, reason: 'healthUnavailable' })
    );
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

  it('health probe REJECTS → "health unavailable", never a false "install the CLI"', async () => {
    const client = createMockClient({
      ai: { activeConfig: async () => ({ activeProvider: 'claude-code', providers: {} }) },
      system: { health: () => Promise.reject(new Error('health probe unreachable')) },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() =>
      expect(result.current).toEqual({ canUse: false, reason: 'healthUnavailable' })
    );
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

  it('key query still in flight → "checking" (no reason), never a premature "add an API key"', async () => {
    // The cloud mirror of the local-server "health probe in flight" case
    // above, and the same class of bug: `providerKeyQuery.data?.has ?? false`
    // read a not-yet-loaded `undefined` as a settled "there is no key", so
    // every consumer was handed `addApiKey` — a concrete instruction — for a
    // question nobody had answered yet. `ModelSelector` had to work around it
    // locally (`activeKeyLoading`); consumers of the hook could not.
    const client = createMockClient({
      ai: {
        activeConfig: async () => ({
          activeProvider: 'openai',
          providers: { openai: { model: 'gpt-4o' } },
        }),
        // Never resolves, isolating the window where `useActiveConfig` has
        // settled and the key status has not.
        hasProviderKey: () => new Promise<{ has: boolean }>(() => {}),
      },
      system: { health: async () => readyHealth() },
    });
    const { result, queryClient } = renderHookWithClient(() => useCanUseAI(), { client });
    // `{ canUse: false }` with no reason is ALSO the first synchronous
    // render's value (`useActiveConfig`'s own cold-boot branch), so require
    // that query to have SETTLED in the same assertion — this can only pass
    // from inside the cloud branch, past the branch that returns it for free.
    await waitFor(() => {
      expect(queryClient.getQueryState(['ai', 'activeConfig'])?.status).toBe('success');
      expect(result.current).toEqual({ canUse: false });
    });
  });

  it('key query REJECTS → "health unavailable", never a false "add an API key"', async () => {
    // The failure twin of the in-flight case above. A rejected keyring read is
    // not an answer either: `data` stays `undefined` forever (retries are off),
    // so `?? false` read the FAILURE as a settled "there is no key" and told a
    // user who has one to go add one. `healthUnavailable`'s copy ("Couldn't
    // check AI status. Try again in a moment.") already says exactly this, so
    // the fix needs no new reason — both consumers already map it.
    const client = createMockClient({
      ai: {
        activeConfig: async () => ({
          activeProvider: 'openai',
          providers: { openai: { model: 'gpt-4o' } },
        }),
        // Rejects rather than hanging: a model IS configured, so without the
        // `isError` guard this lands on `addApiKey`, not on a "checking" state.
        hasProviderKey: () => Promise.reject(new Error('keyring read failed')),
      },
      system: { health: async () => readyHealth() },
    });
    const { result } = renderHookWithClient(() => useCanUseAI(), { client });
    await waitFor(() =>
      expect(result.current).toEqual({ canUse: false, reason: 'healthUnavailable' })
    );
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
