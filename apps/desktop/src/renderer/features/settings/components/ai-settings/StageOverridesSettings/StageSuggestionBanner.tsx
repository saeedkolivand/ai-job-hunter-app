import { Sparkles } from 'lucide-react';
import { useState } from 'react';

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
 *
 * "Not now" dismisses it for the session-and-installed-set: re-offering the
 * same advice on every visit is nagging, but a NEW model on disk is new
 * information and re-arms it.
 */
export function StageSuggestionBanner({
  activeProvider,
  activeModel,
  installedModels,
  overrides,
  onApplied,
}: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const setStageOverride = useSetStageOverride();
  const inspections = useModelInspections(installedModels);
  const [dismissedFor, setDismissedFor] = useState<string | null>(null);

  const suggestion = suggestExtractionModel({
    activeProvider,
    activeModel,
    installedModels,
    inspections: inspections.byModel,
    overrides,
  });

  // Keyed on the installed set: installing something new re-arms the offer.
  const installedKey = [...installedModels].sort().join('|');
  if (!suggestion || dismissedFor === installedKey) return null;

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
          <Button variant="ghost" onClick={() => setDismissedFor(installedKey)}>
            {t('settings.ai.stages.suggest.dismiss')}
          </Button>
        </div>
      </div>
    </div>
  );
}
