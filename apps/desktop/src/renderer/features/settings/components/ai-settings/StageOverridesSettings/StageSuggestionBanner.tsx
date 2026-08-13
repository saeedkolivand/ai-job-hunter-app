import { Sparkles } from 'lucide-react';

import type { AiStageOverride, PipelineStage } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, useNotification } from '@ajh/ui';

import { useModelInspections, useSetStageOverride } from '@/services';

import { suggestExtractionModel } from './suggest-stage-models';

interface Props {
  activeProvider?: string;
  activeModel?: string;
  installedModels: string[];
  overrides: Record<string, AiStageOverride>;
  /** Called with the stages that were pinned, so the host can take focus off
   *  the button that is about to unmount with this banner. */
  onApplied?: (stages: PipelineStage[]) => void;
  /**
   * Decline handler. The "Not now" button exists ONLY when this is provided —
   * dismissal is the host's state, never this component's: a host that decides
   * what to render in the banner's place (the advisor's "nothing to change"
   * line) cannot see state hidden in here, and used to render neither the offer
   * nor its fallback.
   */
  onDismiss?: () => void;
}

/**
 * "A smaller installed model would do the extraction steps" — a SUGGESTION with
 * a one-click accept, never an automatic switch (the same contract as
 * `LocalModelLimits`' "Use suggested"). Renders nothing when there is nothing
 * honest to suggest.
 *
 * Sizes come from `/api/show` (the same query the advisor uses, so it is one
 * cached fetch, not two) rather than from the model name — a name-parsed size
 * once made this recommend a 70B model as the smaller option.
 */
export function StageSuggestionBanner({
  activeProvider,
  activeModel,
  installedModels,
  overrides,
  onApplied,
  onDismiss,
}: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const setStageOverride = useSetStageOverride();
  // Gated: a cloud user can never receive a suggestion, so probing every
  // installed model's `/api/show` for one is pure cost.
  const canSuggest = activeProvider === 'ollama' && Boolean(activeModel);
  const inspections = useModelInspections(canSuggest ? installedModels : []);

  const suggestion = suggestExtractionModel({
    activeProvider,
    activeModel,
    installedModels,
    inspections: inspections.byModel,
    overrides,
  });

  if (!suggestion) return null;

  const stageNames = suggestion.stages
    .map((stage) => t(`settings.ai.stages.names.${stage}`))
    .join(', ');

  const apply = async () => {
    let done = 0;
    try {
      // Sequential: each write returns the fresh map, and a rejection must stop
      // the rest rather than leave a half-applied suggestion behind quietly.
      for (const stage of suggestion.stages) {
        await setStageOverride.mutateAsync({
          stage,
          provider: 'ollama',
          model: suggestion.model,
        });
        done += 1;
      }
      notify.success({
        message: t('settings.ai.stages.suggest.applied', {
          count: done,
          model: suggestion.model,
        }),
      });
      onApplied?.(suggestion.stages);
    } catch (err) {
      // Say how far it got: "failed" would hide that some stages ARE pinned now.
      notify.error({
        message: t('settings.ai.stages.suggest.partial', {
          done,
          total: suggestion.stages.length,
          reason: err instanceof Error ? err.message : String(err),
        }),
      });
      if (done > 0) onApplied?.(suggestion.stages.slice(0, done));
    }
  };

  return (
    <div className="mb-3 rounded-lg border border-brand/25 bg-brand/[0.06] px-3 py-2.5">
      <div className="flex items-start gap-2">
        <Sparkles size={13} className="mt-0.5 shrink-0 text-brand-soft" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <div className="text-xs font-medium text-foreground/80">
            {t('settings.ai.stages.suggest.title')}
          </div>
          <p className="mt-1 text-xs leading-relaxed text-foreground/60">
            {t('settings.ai.stages.suggest.body', {
              model: suggestion.model,
              candidateB: suggestion.candidateB,
              active: activeModel,
              activeB: suggestion.activeB,
            })}
          </p>
          <p className="mt-1 text-[11px] text-foreground/60">
            {t('settings.ai.stages.suggest.appliesTo', { stages: stageNames })}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <Button
            variant="glass"
            loading={setStageOverride.isPending}
            disabled={setStageOverride.isPending}
            onClick={() => void apply()}
          >
            {t('settings.ai.stages.suggest.apply')}
          </Button>
          {onDismiss && (
            <Button variant="ghost" onClick={onDismiss}>
              {t('settings.ai.stages.suggest.dismiss')}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
