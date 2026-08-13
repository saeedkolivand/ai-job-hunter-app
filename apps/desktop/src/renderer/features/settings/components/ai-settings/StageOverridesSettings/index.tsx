import { AlertTriangle, Layers } from 'lucide-react';
import { useRef, useState } from 'react';

import type { PipelineStage } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, SettingsSection, useNotification } from '@ajh/ui';

import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useClearStageOverride, useStageOverrides } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';
import type { Model } from '@/types';

import { ModelAdvisor } from '../ModelAdvisor';
import {
  providerNeedsModel,
  resolveActiveProblem,
  resolveStageRouting,
  type StageRouting,
} from './stage-routing';
import { StageOverrideEditor } from './StageOverrideEditor';
import { StageSuggestionBanner } from './StageSuggestionBanner';
import { installedSetKey } from './suggest-stage-models';

interface Props {
  /** Active provider/model — what an un-overridden stage runs on. */
  activeProvider?: string;
  activeModel?: string;
  ollamaModels: Model[];
  /** Live key/health status per provider, from the provider rows above (one
   *  source of truth for "is this provider reachable", never a second probe). */
  isConfigured: (provider: string) => boolean;
  configuredBaseUrl?: string;
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
}: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const { data: overrides = {} } = useStageOverrides();
  const clearStageOverride = useClearStageOverride();
  const [editing, setEditing] = useState<PipelineStage | null>(null);
  const [advisorOpen, setAdvisorOpen] = useState(false);
  const [dismissedFor, setDismissedFor] = useState<string | null>(null);
  // Per-row "Change" buttons, so focus can come back to the control that opened
  // the editor once it unmounts (WCAG 2.4.3 — otherwise focus drops to <body>).
  const changeButtons = useRef<Record<string, HTMLButtonElement | null>>({});

  const rows = resolveStageRouting({
    overrides,
    active: { provider: activeProvider, model: activeModel },
    isConfigured,
  });
  const activeProblem = resolveActiveProblem({
    active: { provider: activeProvider, model: activeModel },
    isConfigured,
  });

  const installedModels = ollamaModels.map((m) => m.name);

  /** The advisor only has local-model answers to give. */
  const advisorApplies = activeProvider === 'ollama' || ollamaModels.length > 0;

  /** A declined suggestion stays declined for the same installed set. */
  const suggestionDismissed = dismissedFor === installedSetKey(installedModels);

  /** After the suggestion is accepted its button disappears with it, so send
   *  focus to the first stage it pinned. */
  const focusFirstAppliedRow = (stages: PipelineStage[]) =>
    changeButtons.current[stages[0] ?? '']?.focus();

  const closeEditor = (stage: PipelineStage) => {
    setEditing(null);
    changeButtons.current[stage]?.focus();
  };

  const clear = (stage: PipelineStage) =>
    clearStageOverride.mutate(stage, {
      // Clearing the override unmounts THIS button (it only exists while the
      // row is overridden) — the same WCAG 2.4.3 drop the editor guards against.
      onSuccess: () => changeButtons.current[stage]?.focus(),
      onError: (err) =>
        notify.error({ message: t('settings.ai.stages.clearFailed', { reason: err.message }) }),
    });

  const effectiveLabel = (row: StageRouting) =>
    row.override
      ? // An empty model means "the CLI's own default" only where a CLI agent
        // runs it. On a cloud provider it means NO model — labelling that
        // "CLI default" told the user a broken row was configured, right next
        // to the warning saying it will fail.
        `${providerLabel(row.override.provider)} · ${
          row.override.model ||
          t(
            providerNeedsModel(row.override.provider)
              ? 'settings.ai.stages.noModel'
              : 'settings.ai.stages.cliDefaultModel'
          )
        }`
      : activeModel
        ? t('settings.ai.stages.defaultModel', { model: activeModel })
        : t('settings.ai.stages.defaultNoModel');

  return (
    <SettingsSection icon={Layers} label={t('settings.ai.stages.title')}>
      <div className="mb-3 flex items-start justify-between gap-3">
        <p className="text-xs leading-relaxed text-foreground/50">
          {t('settings.ai.stages.description')}
        </p>
        {/* Ollama-only by construction: every question the advisor answers is
            about a local model file (window size, VRAM fit, thinking ratio).
            A cloud-only setup would open four steps of empty states. */}
        {advisorApplies && (
          <Button variant="ghost" className="shrink-0" onClick={() => setAdvisorOpen(true)}>
            {t('settings.ai.advisor.open')}
          </Button>
        )}
      </div>

      {advisorApplies && (
        <ModelAdvisor
          open={advisorOpen}
          onClose={() => setAdvisorOpen(false)}
          activeProvider={activeProvider}
          activeModel={activeModel}
          installedModels={installedModels}
        />
      )}

      {/* Suggestion only — accepting it is a click, never a silent switch.
          Dismissal is THIS card's state: the banner unmounts when declined, so
          it cannot hold the memory of having been declined. */}
      {!suggestionDismissed && (
        <StageSuggestionBanner
          activeProvider={activeProvider}
          activeModel={activeModel}
          installedModels={installedModels}
          overrides={overrides}
          onApplied={focusFirstAppliedRow}
          onDismiss={() => setDismissedFor(installedSetKey(installedModels))}
        />
      )}

      {/* ONE line for the active provider, not the same sentence on every row:
          the fix is a single action, and "put this stage back on the default"
          is not advice you can follow on a row already on the default. */}
      {activeProblem && (
        <p className="mb-3 flex items-start gap-1.5 text-xs text-amber-400">
          <AlertTriangle size={12} className="mt-0.5 shrink-0" aria-hidden="true" />
          <span>
            {activeProblem === 'unconfigured'
              ? t('settings.ai.stages.warnActiveUnconfigured', {
                  provider: providerLabel(activeProvider) || t('settings.ai.stages.noProvider'),
                })
              : t('settings.ai.stages.warnActiveNoModel', {
                  provider: providerLabel(activeProvider),
                })}
          </span>
        </p>
      )}

      <div className="space-y-2">
        {rows.map((row) => (
          <div
            key={row.stage}
            // A stable hook for tests + the settings search: the row must not be
            // findable only by a Tailwind class that styling is free to change.
            data-stage-row={row.stage}
            className="rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2.5"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="min-w-0">
                <div className="text-sm font-medium text-foreground/80">
                  {t(`settings.ai.stages.names.${row.stage}`)}
                </div>
                <div className="mt-0.5 text-[11px] text-foreground/60">
                  {t(`settings.ai.stages.descriptions.${row.stage}`)}
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span
                  className={
                    row.override ? 'text-[11px] text-brand-soft' : 'text-[11px] text-foreground/60'
                  }
                >
                  {effectiveLabel(row)}
                </span>
                <Button
                  ref={(el) => {
                    changeButtons.current[row.stage] = el;
                  }}
                  variant="ghost"
                  onClick={() =>
                    editing === row.stage ? closeEditor(row.stage) : setEditing(row.stage)
                  }
                  aria-expanded={editing === row.stage}
                  aria-controls={`stage-editor-${row.stage}`}
                  // Seven controls all named "Change" say nothing about WHICH
                  // step they change. The stage name is interpolated by the
                  // translation so word order stays the translator's call.
                  aria-label={t(
                    editing === row.stage
                      ? 'settings.ai.stages.changeCloseFor'
                      : 'settings.ai.stages.changeFor',
                    { stage: t(`settings.ai.stages.names.${row.stage}`) }
                  )}
                >
                  {editing === row.stage
                    ? t('settings.ai.stages.changeClose')
                    : t('settings.ai.stages.change')}
                </Button>
                {row.override && (
                  <Button
                    variant="ghost"
                    disabled={clearStageOverride.isPending}
                    onClick={() => clear(row.stage)}
                    aria-label={t('settings.ai.stages.resetFor', {
                      stage: t(`settings.ai.stages.names.${row.stage}`),
                    })}
                  >
                    {t('settings.ai.stages.reset')}
                  </Button>
                )}
              </div>
            </div>

            {/* An override the backend will REFUSE at run time (it never falls
                back to the active provider) — said here, before the run. */}
            {row.problem && (
              <p className="mt-2 flex items-start gap-1.5 text-xs text-amber-400">
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
                id={`stage-editor-${row.stage}`}
                stage={row.stage}
                override={row.override}
                fallbackProvider={row.provider}
                ollamaModels={ollamaModels}
                isConfigured={isConfigured}
                configuredBaseUrl={configuredBaseUrl}
                onDone={() => closeEditor(row.stage)}
              />
            )}
          </div>
        ))}
      </div>
    </SettingsSection>
  );
}
