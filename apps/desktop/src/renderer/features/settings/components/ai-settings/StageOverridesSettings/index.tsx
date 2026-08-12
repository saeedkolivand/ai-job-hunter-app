import { AlertTriangle, Layers } from 'lucide-react';
import { useState } from 'react';

import type { PipelineStage } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, SettingsSection, useNotification } from '@ajh/ui';

import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useClearStageOverride, useStageOverrides } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';
import type { Model } from '@/types';

import { resolveStageRouting, type StageRouting } from './stage-routing';
import { StageOverrideEditor } from './StageOverrideEditor';
import { StageSuggestionBanner } from './StageSuggestionBanner';

interface Props {
  /** Active provider/model — what an un-overridden stage runs on. */
  activeProvider?: string;
  activeModel?: string;
  ollamaModels: Model[];
  /** Live key/health status per provider, from the provider rows above (one
   *  source of truth for "is this provider reachable", never a second probe). */
  isConfigured: (provider: string) => boolean;
  configuredBaseUrl?: string;
  /** Rendered under the intro — the suggested-defaults banner and the advisor
   *  entry point, kept out of this component's own concern. */
  children?: React.ReactNode;
}

const providerLabel = (provider?: string) =>
  (provider && PROVIDERS[provider as AiProvider]?.label) || provider || '';

/**
 * Per-stage model overrides: which model runs each step of a staged run.
 *
 * Renders the EFFECTIVE model per stage and lets one stage at a time be
 * repointed. Stages with no override say "default — <active model>" rather than
 * silently showing the active model as if it were pinned: an absent row means
 * the stage follows the active provider, and writing today's default back as an
 * override would freeze that stage on today's model forever.
 */
export function StageOverridesSettings({
  activeProvider,
  activeModel,
  ollamaModels,
  isConfigured,
  configuredBaseUrl,
  children,
}: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const { data: overrides = {} } = useStageOverrides();
  const clearStageOverride = useClearStageOverride();
  const [editing, setEditing] = useState<PipelineStage | null>(null);

  const rows = resolveStageRouting({
    overrides,
    active: { provider: activeProvider, model: activeModel },
    isConfigured,
  });

  const clear = (stage: PipelineStage) =>
    clearStageOverride.mutate(stage, {
      onError: (err) =>
        notify.error({ message: t('settings.ai.stages.clearFailed', { reason: err.message }) }),
    });

  const effectiveLabel = (row: StageRouting) =>
    row.override
      ? `${providerLabel(row.override.provider)} · ${row.override.model || t('settings.ai.stages.cliDefaultModel')}`
      : activeModel
        ? t('settings.ai.stages.defaultModel', { model: activeModel })
        : t('settings.ai.stages.defaultNoModel');

  return (
    <SettingsSection icon={Layers} label={t('settings.ai.stages.title')}>
      <p className="mb-3 text-xs leading-relaxed text-foreground/50">
        {t('settings.ai.stages.description')}
      </p>

      {children}

      {/* Suggestion only — accepting it is a click, never a silent switch. */}
      <StageSuggestionBanner
        activeProvider={activeProvider}
        activeModel={activeModel}
        installedModels={ollamaModels.map((m) => m.name)}
        overrides={overrides}
      />

      <div className="space-y-2">
        {rows.map((row) => (
          <div
            key={row.stage}
            className="rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2.5"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="min-w-0">
                <div className="text-sm font-medium text-foreground/80">
                  {t(`settings.ai.stages.names.${row.stage}`)}
                </div>
                <div className="mt-0.5 text-[11px] text-foreground/40">
                  {t(`settings.ai.stages.descriptions.${row.stage}`)}
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span
                  className={
                    row.override ? 'text-[11px] text-brand-soft' : 'text-[11px] text-foreground/45'
                  }
                >
                  {effectiveLabel(row)}
                </span>
                <Button
                  variant="ghost"
                  onClick={() => setEditing(editing === row.stage ? null : row.stage)}
                  aria-expanded={editing === row.stage}
                >
                  {t('settings.ai.stages.change')}
                </Button>
                {row.override && (
                  <Button
                    variant="ghost"
                    disabled={clearStageOverride.isPending}
                    onClick={() => clear(row.stage)}
                  >
                    {t('settings.ai.stages.reset')}
                  </Button>
                )}
              </div>
            </div>

            {/* An override the backend will REFUSE at run time (it never falls
                back to the active provider) — said here, before the run. */}
            {row.problem && (
              <p className="mt-2 flex items-start gap-1.5 text-xs text-amber-400/80">
                <AlertTriangle size={12} className="mt-0.5 shrink-0" aria-hidden="true" />
                <span>
                  {row.problem === 'unconfigured'
                    ? t('settings.ai.stages.warnUnconfigured', {
                        provider: providerLabel(row.provider) || t('settings.ai.stages.noProvider'),
                      })
                    : t('settings.ai.stages.warnNoModel', {
                        provider: providerLabel(row.provider),
                      })}
                </span>
              </p>
            )}

            {editing === row.stage && (
              <StageOverrideEditor
                stage={row.stage}
                override={row.override}
                fallbackProvider={row.provider}
                ollamaModels={ollamaModels}
                isConfigured={isConfigured}
                configuredBaseUrl={configuredBaseUrl}
                onDone={() => setEditing(null)}
              />
            )}
          </div>
        ))}
      </div>
    </SettingsSection>
  );
}
