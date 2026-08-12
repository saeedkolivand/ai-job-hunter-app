import { useState } from 'react';

import type { AiStageOverride, PipelineStage } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Dropdown, NumberField, Switch, useNotification } from '@ajh/ui';

import { PROVIDER_ORDER, PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useListProviderModels, useSetStageOverride } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';
import type { Model } from '@/types';

import { providerNeedsModel } from './stage-routing';

/** Backend bounds for `contextWindow` (`validate_context_window`). */
const CTX_MIN = 512;
const CTX_MAX = 131_072;
const CTX_DEFAULT = 8192;

interface Props {
  /** Wired to the row button's `aria-controls`. */
  id: string;
  stage: PipelineStage;
  /** The override being edited, when the stage already has one. */
  override?: AiStageOverride;
  /** Seeded from the active config so "change one thing" doesn't start empty. */
  fallbackProvider?: string;
  ollamaModels: Model[];
  /** Live key/health status, shared with the provider rows above. */
  isConfigured: (provider: string) => boolean;
  /** The stored openai-compatible base URL, forwarded with that provider only. */
  configuredBaseUrl?: string;
  onDone: () => void;
}

/**
 * Set ONE stage's provider + model (+ an optional context window).
 *
 * Only the stage being edited mounts this, so exactly one provider model-list
 * query is ever live — the same reason `useProviderKeys` fetches only for the
 * expanded row.
 */
export function StageOverrideEditor({
  id,
  stage,
  override,
  fallbackProvider,
  ollamaModels,
  isConfigured,
  configuredBaseUrl,
  onDone,
}: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const setStageOverride = useSetStageOverride();

  const [provider, setProvider] = useState<string>(
    override?.provider ?? fallbackProvider ?? 'ollama'
  );
  const [model, setModel] = useState<string>(override?.model ?? '');
  const [windowOn, setWindowOn] = useState(override?.contextWindow !== undefined);
  const [contextWindow, setContextWindow] = useState(override?.contextWindow ?? CTX_DEFAULT);

  const kind = PROVIDERS[provider as AiProvider]?.kind;
  const configured = isConfigured(provider);
  // For LISTING models only. It is deliberately not sent when saving: the
  // backend reads the endpoint from the provider's own settings row, so an
  // override always uses the URL Settings displays.
  const baseUrl = provider === 'openai-compatible' ? configuredBaseUrl : undefined;

  // Cloud + CLI agents list through the same IPC path; Ollama has its own local
  // list (already loaded for the provider rows), so it never re-fetches here.
  const { data: fetched } = useListProviderModels(
    provider,
    configured && kind !== 'local-server',
    baseUrl
  );
  const modelOptions =
    kind === 'local-server'
      ? ollamaModels.map((m) => ({ value: m.name, label: m.name }))
      : (fetched?.models ?? []).map((m) => ({ value: m.name, label: m.displayName ?? m.name }));
  // Keep a stored selection that has fallen out of the live catalogue visible,
  // rather than showing the placeholder as if the choice had been lost.
  const options =
    model && !modelOptions.some((o) => o.value === model)
      ? [{ value: model, label: model }, ...modelOptions]
      : modelOptions;

  const needsModel = providerNeedsModel(provider);
  const canSave = configured && (!needsModel || Boolean(model));

  const save = () =>
    setStageOverride.mutate(
      {
        stage,
        provider,
        model: model || undefined,
        contextWindow: windowOn ? contextWindow : undefined,
      },
      {
        onSuccess: onDone,
        // Server-side validation is the authority (cross-family model, base-URL
        // provenance, window bounds) — show what it said rather than guessing
        // client-side which field it disliked.
        onError: (err) =>
          notify.error({
            message: t('settings.ai.stages.saveFailed', { reason: err.message }),
          }),
      }
    );

  return (
    <div
      id={id}
      className="mt-3 space-y-3 rounded-lg border border-foreground/10 bg-foreground/[0.03] p-3"
    >
      <div className="grid gap-3 @sm:grid-cols-2">
        <div className="space-y-1.5">
          <label
            htmlFor={`stage-provider-${stage}`}
            className="text-[11px] font-semibold uppercase tracking-widest text-foreground/60"
          >
            {t('settings.ai.stages.editor.provider')}
          </label>
          <Dropdown
            id={`stage-provider-${stage}`}
            tone="field"
            value={provider}
            options={PROVIDER_ORDER.map((p) => ({ value: p, label: PROVIDERS[p].label }))}
            onChange={(next) => {
              setProvider(next);
              // A model id belongs to exactly one provider family — carrying it
              // across is what `validate_model` rejects server-side.
              setModel('');
            }}
          />
        </div>

        <div className="space-y-1.5">
          <label
            htmlFor={`stage-model-${stage}`}
            className="text-[11px] font-semibold uppercase tracking-widest text-foreground/60"
          >
            {t('settings.ai.stages.editor.model')}
          </label>
          <Dropdown
            id={`stage-model-${stage}`}
            tone="field"
            value={model}
            options={options}
            disabled={!configured}
            placeholder={
              needsModel
                ? t('settings.ai.stages.editor.modelPlaceholder')
                : t('settings.ai.stages.editor.modelCliDefault')
            }
            onChange={setModel}
          />
        </div>
      </div>

      {!configured && (
        <p className="text-xs text-amber-400">
          {t('settings.ai.stages.editor.providerUnconfigured', {
            provider: PROVIDERS[provider as AiProvider]?.label ?? provider,
          })}
        </p>
      )}

      <div className="border-t border-foreground/10 pt-3">
        <Switch
          label={t('settings.ai.stages.editor.setWindow')}
          checked={windowOn}
          onCheckedChange={setWindowOn}
        />
        {windowOn ? (
          <div className="mt-2 max-w-[12rem]">
            <NumberField
              value={contextWindow}
              onChange={setContextWindow}
              min={CTX_MIN}
              max={CTX_MAX}
              step={512}
              fallback={CTX_DEFAULT}
              aria-label={t('settings.ai.stages.editor.contextWindow')}
            />
          </div>
        ) : (
          <p className="mt-1.5 text-xs text-foreground/60">
            {t('settings.ai.stages.editor.windowDefault')}
          </p>
        )}
      </div>

      <div className="flex items-center gap-2">
        <Button variant="glass" disabled={!canSave || setStageOverride.isPending} onClick={save}>
          {t('settings.ai.stages.editor.save')}
        </Button>
        <Button variant="ghost" onClick={onDone}>
          {t('settings.ai.stages.editor.cancel')}
        </Button>
      </div>
    </div>
  );
}
