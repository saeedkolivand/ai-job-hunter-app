import { AlertTriangle, Cpu, Loader2 } from 'lucide-react';
import { useQueries } from '@tanstack/react-query';

import { useTranslation } from '@ajh/translations';
import { Dropdown } from '@ajh/ui';

import { EffortPicker } from '@/components/ui/EffortPicker';
import { getModelGuidance } from '@/lib/ai-providers/model-guidance';
import { isProviderConfigured, PROVIDER_ORDER, PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useAppClient } from '@/providers/AppClientProvider';
import {
  fetchProviderModelsWithCache,
  useActiveConfig,
  useAIModels,
  useConfigureActiveProvider,
  useHasProviderKey,
  useSystemHealth,
} from '@/services';
import { keys } from '@/services/query-client';
import type { AiProvider } from '@/store/preferences-schema';
import type { Model } from '@/types';

import { buildModelOptions } from './build-options';

interface ModelSelectorProps {
  className?: string;
}

/**
 * Provider + model picker, driven by the provider registry (no hardcoded provider
 * list). Groups models by provider `kind`: Ollama (local server) and cloud lists
 * are fetched; CLI agents (Claude Code / Codex / Gemini CLI) contribute their
 * curated aliases when their binary is detected.
 */
export function ModelSelector({ className }: ModelSelectorProps) {
  const { t } = useTranslation();
  const api = useAppClient();
  const { data: modelList = [], isLoading: ollamaLoading } = useAIModels();
  const ollamaModels = modelList as Model[];
  // Routing is backend-owned (task #16); writes go through the setters, reads
  // through the boot-prefetched active-config query.
  const configureProvider = useConfigureActiveProvider();
  const { data: providerConfig } = useActiveConfig();
  const activeProvider = (providerConfig?.activeProvider ?? 'ollama') as AiProvider;
  const activeProviderModel = providerConfig?.providers?.[activeProvider]?.model ?? '';

  // Cloud key + model status, driven off the registry (no hardcoded provider
  // list) so a new cloud provider — e.g. Ollama Cloud — is picked up with no
  // change here. Custom base URL only applies to the OpenAI-compatible provider.
  const cloudProviders = PROVIDER_ORDER.filter((p) => PROVIDERS[p].kind === 'cloud');
  const baseUrlFor = (p: AiProvider): string | undefined =>
    p === 'openai-compatible'
      ? providerConfig?.providers?.['openai-compatible']?.baseUrl
      : undefined;

  const keyQueries = useQueries({
    queries: cloudProviders.map((p) => ({
      queryKey: [...keys.ai.models, 'provider-key', p],
      queryFn: () => api.ai.hasProviderKey({ provider: p }),
      staleTime: 30_000,
    })),
  });
  const connected = new Map<AiProvider, boolean>(
    cloudProviders.map((p, i) => [p, keyQueries[i]?.data?.has ?? false])
  );
  // `openai-compatible` is the one cloud provider the backend lists models for
  // without a bearer header (LM Studio / vLLM are keyless) — but only once
  // it's actually configured (a stored base URL or key); every other provider
  // still needs a stored key before it can fetch.
  const canFetchModels = (p: AiProvider): boolean =>
    isProviderConfigured(p, connected.get(p) ?? false, baseUrlFor(p));

  const modelQueries = useQueries({
    queries: cloudProviders.map((p) => ({
      queryKey: [...keys.ai.models, 'provider-models', p, baseUrlFor(p) ?? ''],
      // DISPLAY purpose, explicitly — the picker may show a cache-served list
      // (with a "cached" note, below) since a stale-but-disclosed list beats
      // an empty one here; onboarding's verification gate is the one call
      // site that must NOT allow this (see CloudProviderPanel).
      queryFn: () =>
        fetchProviderModelsWithCache(api, p, baseUrlFor(p), { allowCacheFallback: true }),
      enabled: canFetchModels(p),
      staleTime: 300_000,
      // A rejection now means the live fetch failed AND no cached list was
      // available — not transient, so don't re-pay the backend's own timeout
      // on every retry, on every mount.
      retry: false,
    })),
  });
  const cloudModelEntries = new Map(
    cloudProviders.map((p, i) => [p, modelQueries[i]?.data?.models ?? []])
  );

  // CLI-agent availability (binary detected) from the system health probe.
  // `healthLoading` also drives the warning suppression for cli-agent providers
  // (their option source is this probe), so we don't false-warn on first paint.
  const { data: health, isLoading: healthLoading } = useSystemHealth();
  const cliDetected = (id: string) => health?.cliAgents?.[id]?.detected ?? false;

  // Grouped options, derived from the registry by provider kind (pure helper).
  const options = buildModelOptions(PROVIDER_ORDER, PROVIDERS, {
    ollamaModels: ollamaModels.map((m) => m.name),
    cliDetected,
    cloudConnected: canFetchModels,
    cloudModels: (p) => cloudModelEntries.get(p) ?? [],
  });

  // Current selection as "provider||model" — every provider (Ollama included) now
  // stores its model in the backend per-provider config.
  const selectedValue = activeProviderModel ? `${activeProvider}||${activeProviderModel}` : '';

  // Warn whenever the dropdown can't show a real, selectable model for the current
  // selection — i.e. the stored value isn't a visible option (no model picked,
  // uninstalled Ollama model, stale/removed cloud model, CLI default) so the
  // Dropdown falls back to its placeholder. A model must ALWAYS be visibly
  // selected, so this fires for EVERY provider kind (Ollama, cloud, CLI agent).
  // We only suppress it while the active provider's own option source is still
  // loading, to avoid a false warning on first paint.
  const selectedModelVisible = options.some((o) => o.value === selectedValue);
  const activeCloudIndex = cloudProviders.indexOf(activeProvider);
  const activeIsCloud = PROVIDERS[activeProvider]?.kind === 'cloud';
  // `openai-compatible` can fetch keylessly ONLY once it has a stored base
  // URL — in that case its readiness never depends on the key-status query at
  // all (waiting on it would wait on data it doesn't need). But a user who
  // authenticated it with a stored KEY instead (no base URL) DOES depend on
  // that query settling, same as every other cloud provider — so the key
  // query is irrelevant to `openai-compatible` only when a base URL is
  // already configured; `isProviderConfigured` mirrors this exact condition.
  // For every provider where the key query IS relevant, its own loading state
  // must be tracked separately from "no key" (below): while it's still
  // resolving, `canFetchModels` reads the not-yet-loaded `false` default,
  // which is NOT the same as "we checked — there's no key".
  const activeKeyRequired =
    activeIsCloud && (activeProvider !== 'openai-compatible' || !baseUrlFor(activeProvider));
  const activeKeyLoading = activeKeyRequired && Boolean(keyQueries[activeCloudIndex]?.isLoading);
  const modelsLoading = (() => {
    switch (PROVIDERS[activeProvider]?.kind) {
      case 'local-server':
        return ollamaLoading;
      case 'cli-agent':
        return healthLoading;
      case 'cloud':
        return activeCloudIndex >= 0
          ? activeKeyLoading || Boolean(modelQueries[activeCloudIndex]?.isLoading)
          : false;
      default:
        return false;
    }
  })();
  const showModelWarning = !modelsLoading && !selectedModelVisible;
  const warningKey = selectedValue === '' ? 'models.noModelSelected' : 'models.modelUnavailable';

  // Cloud model-list health for the ACTIVE provider — a distinct concern from
  // `showModelWarning` above (that's about the current *selection*; this is
  // about whether the *list itself* loaded, and from where). ONE status,
  // computed by a single ordered if/else chain rather than four independent
  // booleans each gating its own JSX block — that shape is how "cached" and
  // "needs key" rendered simultaneously last round (a stale `cached: true`
  // result survived a key being removed, and only three of the four booleans
  // were gated on `activeCloudNeedsKey`). A single chain makes every state
  // mutually exclusive BY CONSTRUCTION: a future fifth state is just another
  // branch, and it structurally cannot co-render with an earlier one.
  const activeCloudQuery = activeCloudIndex >= 0 ? modelQueries[activeCloudIndex] : undefined;
  const activeCloudStatus: 'idle' | 'needsKey' | 'loading' | 'error' | 'cached' = !activeIsCloud
    ? 'idle'
    : activeKeyLoading
      ? 'loading' // key query itself unsettled — NOT "checked, no key"
      : !canFetchModels(activeProvider)
        ? 'needsKey'
        : modelsLoading
          ? 'loading'
          : activeCloudQuery?.isError
            ? 'error'
            : (activeCloudQuery?.data?.cached ?? false)
              ? 'cached'
              : 'idle';
  const activeCloudErrorMessage =
    activeCloudStatus === 'error'
      ? activeCloudQuery?.error instanceof Error
        ? activeCloudQuery.error.message
        : String(activeCloudQuery?.error)
      : undefined;

  // "Which model for what" hint (#6) for the current selection — derived from the
  // model name + provider kind, so new models are covered with no code change.
  const selectedModelName = selectedValue ? (selectedValue.split('||')[1] ?? '') : '';
  const guidance = selectedModelName
    ? getModelGuidance(selectedModelName, PROVIDERS[activeProvider]?.kind ?? 'local-server')
    : null;

  const handleModelChange = (value: string) => {
    const [provider, model] = value.split('||');
    if (!provider || !model) return;
    // Set the provider's model AND make it active in one backend write.
    configureProvider.mutate({ provider, model });
  };

  const dropdown = (
    <Dropdown
      options={options}
      value={selectedValue}
      onChange={handleModelChange}
      placeholder="Select a model…"
      icon={<Cpu size={13} />}
    />
  );

  return (
    <div className={className}>
      <div className="flex items-center gap-2">
        <div className="flex-1 min-w-0 w-full">
          {showModelWarning ? (
            <div className="border border-amber-400/30 bg-amber-400/5 rounded-lg p-2">
              {dropdown}
              <p
                role="status"
                aria-live="polite"
                className="mt-1.5 flex items-center gap-1 text-[10px] text-amber-400/80"
              >
                <AlertTriangle size={12} className="shrink-0" />
                {t(warningKey)}
              </p>
            </div>
          ) : (
            dropdown
          )}
        </div>
      </div>
      {/* Reasoning effort for the selected model. Rendered unconditionally —
          EffortPicker returns null unless the backend reports `effortLevels`
          for this model, so a non-reasoning model shows nothing here. This one
          insertion is what puts the control on every model-picking surface in
          the app; do not duplicate it at the call sites. */}
      <EffortPicker
        provider={activeProvider}
        model={activeProviderModel}
        baseUrl={baseUrlFor(activeProvider)}
        compact
      />
      {activeCloudStatus === 'needsKey' && (
        <p role="status" aria-live="polite" className="mt-1.5 text-[10px] text-foreground/40">
          {/* `openai-compatible` needs a base URL, not a key — this is the
              DEFAULT state for anyone who has never configured it (the
              population the keyless carve-out exists for), so telling them
              to add a key instead points at the one thing that won't help. */}
          {t(
            activeProvider === 'openai-compatible'
              ? 'models.cloud.addUrlToLoad'
              : 'models.cloud.addKeyToLoad'
          )}
        </p>
      )}
      {activeCloudStatus === 'loading' && (
        <p
          role="status"
          aria-live="polite"
          className="mt-1.5 flex items-center gap-1 text-[10px] text-foreground/40"
        >
          <Loader2 size={11} className="shrink-0 animate-spin" />
          {t('settings.aiModel.loading')}
        </p>
      )}
      {activeCloudStatus === 'error' && activeCloudErrorMessage && (
        <p role="alert" className="mt-1.5 flex items-start gap-1 text-[10px] text-red-400/80">
          <AlertTriangle size={11} className="mt-px shrink-0" />
          {t('models.cloud.fetchFailed', { message: activeCloudErrorMessage })}
        </p>
      )}
      {activeCloudStatus === 'cached' && (
        <p role="status" aria-live="polite" className="mt-1.5 text-[10px] text-foreground/40">
          {t('models.cloud.cachedList')}
        </p>
      )}
      {guidance && (
        <p className="mt-1.5 flex flex-wrap items-center gap-x-1.5 gap-y-0.5 text-[10px] leading-relaxed text-foreground/40">
          <span className="rounded bg-muted px-1.5 py-0.5 font-medium text-foreground/55">
            {t(`models.tier.${guidance.tier}`)}
          </span>
          <span className="text-foreground/55">{t(`models.guidance.task.${guidance.task}`)}</span>
          <span className="text-foreground/30">· {t(`models.guidance.kind.${guidance.kind}`)}</span>
        </p>
      )}
    </div>
  );
}

export function useSelectedModel(): string {
  // The active provider's model is backend-resolved (task #16); `data.model` is
  // the active provider's own model (empty for a not-yet-configured provider).
  const { data } = useActiveConfig();
  const model = data?.model ?? '';
  // Strip a provider prefix if one slipped in (e.g. "ollama||gpt-oss" -> "gpt-oss").
  return model.includes('||') ? (model.split('||')[1] ?? model) : model;
}

export function useSelectedProvider(): string {
  return useActiveConfig().data?.activeProvider ?? 'ollama';
}

export function useCanUseAI(): { canUse: boolean; reason?: string } {
  const { data: providerConfig, isPending } = useActiveConfig();
  const activeProvider = (providerConfig?.activeProvider ?? 'ollama') as AiProvider;
  const kind = PROVIDERS[activeProvider]?.kind ?? 'local-server';
  const activeProviderModel = providerConfig?.providers?.[activeProvider]?.model ?? '';

  // Hooks must run unconditionally regardless of which branch applies below.
  const providerKeyQuery = useHasProviderKey(activeProvider);
  const { data: health } = useSystemHealth();

  // Cold boot: while the backend config is first loading, block WITHOUT a concrete
  // reason so the button stays disabled but no scary "no provider" hint flashes
  // (boot-prefetched, so this window is sub-frame in practice).
  if (isPending) return { canUse: false };

  if (kind === 'cloud') {
    if (!(providerKeyQuery.data?.has ?? false)) return { canUse: false, reason: 'addApiKey' };
    if (!activeProviderModel) return { canUse: false, reason: 'selectModel' };
    return { canUse: true };
  }
  if (kind === 'cli-agent') {
    // Keyless; model optional (empty → the tool's default). Only gate on the
    // binary being installed.
    const detected = health?.cliAgents?.[activeProvider]?.detected ?? false;
    return detected ? { canUse: true } : { canUse: false, reason: 'installCli' };
  }
  // local-server (Ollama) — the model now lives in the per-provider config.
  if (!activeProviderModel) return { canUse: false, reason: 'selectModel' };
  return { canUse: true };
}
