/**
 * Stub state + mock factories for the ModelSelector tests. `vi.mock` is hoisted per
 * test file, so each file wires these in with
 * `vi.mock('<pkg>', async () => (await import('./test-support')).<factory>())`.
 * Tests drive the component by mutating `stub` (reset in `beforeEach`).
 */
import { vi } from 'vitest';

export type CloudQueryState = {
  data?: unknown;
  isLoading?: boolean;
  isError?: boolean;
  error?: unknown;
};

const baseline = () => ({
  // Routing (activeProvider + per-provider model) is backend-owned (task #16), read
  // via `useActiveConfig`; the model now lives in `providers[activeProvider].model`
  // for EVERY provider kind (the old `aiModel` Ollama mirror is gone).
  activeProvider: 'ollama',
  activeProviderModel: '',
  // Installed Ollama model names — drives `options` via buildModelOptions, so a test
  // can make the stored selection a visible option (warning suppressed) or not
  // (warning shown).
  ollamaModels: [] as Array<{ name: string }>,
  // System-health probe result — drives CLI-agent option availability (a CLI agent
  // contributes its curated models only when `detected`) and the cli-agent branch of
  // `modelsLoading` (via `isLoading`).
  health: { data: undefined, isLoading: false } as {
    data: { cliAgents?: Record<string, { detected: boolean }> } | undefined;
    isLoading: boolean;
  },
  // `openai-compatible`'s stored base URL — an ALTERNATIVE to a stored key for
  // `canFetchModels`'s "actually configured" check (either satisfies it; see
  // `isProviderConfigured`), not a second requirement on top of one.
  openAiCompatibleBaseUrl: undefined as string | undefined,
  // Per-provider fixtures for the two `useQueries` calls (key-status + model-list).
  cloudKeyQueries: {} as Record<string, CloudQueryState>,
  cloudModelQueries: {} as Record<string, CloudQueryState>,
});

export const stub = baseline();

/** Reset every controllable stub to a deterministic baseline (call in `beforeEach`). */
export function resetStub() {
  Object.assign(stub, baseline());
}

export const translationsMock = () => ({
  useTranslation: () => ({ t: (k: string) => k }),
});

export const appClientMock = (): Record<string, unknown> => ({
  useAppClient: () => ({
    ai: {
      hasProviderKey: vi.fn().mockResolvedValue({ has: false }),
      listProviderModels: vi.fn().mockResolvedValue([]),
    },
  }),
});

/** Service stubs — prevent a QueryClient dependency. */
export const servicesMock = (): Record<string, unknown> => ({
  useActiveConfig: () => {
    // Built via two statements, not one object literal with both
    // `[stub.activeProvider]: {...}` and a literal `'openai-compatible':
    // {...}` key — when `stub.activeProvider === 'openai-compatible'`
    // those are the SAME key, and the second entry would silently win,
    // dropping `model` and leaving `activeProviderModel` `''` regardless of
    // `stub.activeProviderModel`. Assigning `baseUrl` onto the existing
    // entry (spreading whatever's already there) merges instead of overwrites.
    const providers: Record<string, { model?: string; baseUrl?: string }> = {
      [stub.activeProvider]: { model: stub.activeProviderModel },
    };
    providers['openai-compatible'] = {
      ...providers['openai-compatible'],
      baseUrl: stub.openAiCompatibleBaseUrl,
    };
    return {
      data: {
        activeProvider: stub.activeProvider,
        model: stub.activeProviderModel,
        providers,
      },
      isPending: false,
    };
  },
  useConfigureActiveProvider: () => ({ mutate: vi.fn() }),
  useAIModels: () => ({ data: stub.ollamaModels, isLoading: false }),
  useHasProviderKey: () => ({ data: { has: false } }),
  useSystemHealth: () => stub.health,
  // ModelSelector renders EffortPicker, which reads model capabilities. These
  // suites are about the MODEL dropdown; no `effortLevels` means EffortPicker
  // renders null and stays out of the way. Its own behaviour is covered in
  // EffortPicker.test.tsx.
  useModelCapabilities: () => ({ data: undefined }),
});

/**
 * Keep QueryClient et al, stub only `useQueries`.
 *
 * ModelSelector fires two `useQueries` calls (key-status + model-list), each
 * keyed `[...keys.ai.models, 'provider-key' | 'provider-models', provider, ...]`
 * — a per-test fixture keyed by that same `[kind, provider]` pair lets a test
 * express a cloud provider as connected (`provider-key`) while its
 * `provider-models` query is settled in a particular state (fresh / cached /
 * errored), so the picker's cache/error handling is exercised through the real
 * component rather than stubbed away.
 */
export const reactQueryMock = (actual: Record<string, unknown>) => ({
  ...actual,
  useQueries: ({ queries }: { queries: Array<{ queryKey: unknown[] }> }) =>
    queries.map(({ queryKey }) => {
      const [, , kind, provider] = queryKey as [unknown, unknown, string, string];
      const table = kind === 'provider-key' ? stub.cloudKeyQueries : stub.cloudModelQueries;
      return (
        table[provider] ?? {
          data: kind === 'provider-key' ? { has: false } : { models: [], cached: false },
          isLoading: false,
        }
      );
    }),
});
