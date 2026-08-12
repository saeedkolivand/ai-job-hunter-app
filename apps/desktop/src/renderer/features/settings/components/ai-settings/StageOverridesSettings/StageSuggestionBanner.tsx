import { Sparkles } from 'lucide-react';

import type { AiStageOverride } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, useNotification } from '@ajh/ui';

import { useSetStageOverride } from '@/services';

import { suggestExtractionModel } from './suggest-stage-models';

interface Props {
  activeProvider?: string;
  activeModel?: string;
  installedModels: string[];
  overrides: Record<string, AiStageOverride>;
}

/**
 * "A smaller installed model would do the extraction steps" — a SUGGESTION with
 * a one-click accept, never an automatic switch (the same contract as
 * `LocalModelLimits`' "Use suggested"). Renders nothing when there is nothing
 * honest to suggest.
 *
 * The copy names what will change and why, because accepting writes three real
 * override rows: silent magic here would be indistinguishable from the app
 * quietly downgrading the user's model.
 */
export function StageSuggestionBanner({
  activeProvider,
  activeModel,
  installedModels,
  overrides,
}: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const setStageOverride = useSetStageOverride();

  const suggestion = suggestExtractionModel({
    activeProvider,
    activeModel,
    installedModels,
    overrides,
  });
  if (!suggestion) return null;

  const stageNames = suggestion.stages
    .map((stage) => t(`settings.ai.stages.names.${stage}`))
    .join(', ');

  const apply = async () => {
    try {
      // Sequential: each write returns the fresh map, and a rejection must stop
      // the rest rather than leave a half-applied suggestion behind quietly.
      for (const stage of suggestion.stages) {
        await setStageOverride.mutateAsync({
          stage,
          provider: 'ollama',
          model: suggestion.model,
        });
      }
      notify.success({
        message: t('settings.ai.stages.suggest.applied', {
          count: suggestion.stages.length,
          model: suggestion.model,
        }),
      });
    } catch (err) {
      notify.error({
        message: t('settings.ai.stages.saveFailed', {
          reason: err instanceof Error ? err.message : String(err),
        }),
      });
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
          <p className="mt-1 text-xs leading-relaxed text-foreground/55">
            {t('settings.ai.stages.suggest.body', {
              model: suggestion.model,
              active: activeModel,
            })}
          </p>
          <p className="mt-1 text-[11px] text-foreground/40">
            {t('settings.ai.stages.suggest.appliesTo', { stages: stageNames })}
          </p>
        </div>
        <Button
          variant="glass"
          className="shrink-0"
          disabled={setStageOverride.isPending}
          onClick={() => void apply()}
        >
          {t('settings.ai.stages.suggest.apply')}
        </Button>
      </div>
    </div>
  );
}
