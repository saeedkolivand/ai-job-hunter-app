import { Ruler } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { EmptyState, RowSkeleton } from '@ajh/ui';

import { assessModelContextFit, type ContextFitVerdict, worstCaseStage } from './context-fit';
import type { AdvisorStepProps } from './types';

/** One token class per verdict — no `[#RRGGBB]`, and each state has its own
 *  visible treatment rather than only differing by wording. */
const VERDICT_CLASS: Record<ContextFitVerdict, string> = {
  fits: 'text-emerald-400',
  tight: 'text-amber-400',
  'too-small': 'text-red-400',
  unknown: 'text-foreground/60',
};

/**
 * Step 2 — does each model's window hold what the stages actually send?
 *
 * The demand side is the Rust prompt caps (see `context-fit.ts`), so the
 * verdict is a comparison against real bounds, not a rule of thumb. A model
 * with no measured window reads "not measured": the app never assumes a size.
 */
export function ContextFitStep({ ctx }: AdvisorStepProps) {
  const { t, i18n } = useTranslation();

  // No local models at all (a cloud-only setup): say so, rather than leaving
  // intro prose above an empty list and a footnote about truncation below it.
  if (ctx.installedModels.length === 0) {
    return (
      <EmptyState
        icon={Ruler}
        title={t('settings.ai.advisor.fit.emptyTitle')}
        description={t('settings.ai.advisor.fit.emptyDescription')}
      />
    );
  }

  // Which stage is the biggest is COMPUTED, not asserted — `sections` carries
  // four artifacts and beats the draft turn.
  const worst = worstCaseStage();

  return (
    <div className="space-y-3">
      <p className="text-xs leading-relaxed text-foreground/60">
        {t('settings.ai.advisor.fit.intro', {
          stage: t(`settings.ai.stages.names.${worst.stage}`),
          chars: worst.chars.toLocaleString(i18n.language),
          tokens: worst.tokens.toLocaleString(i18n.language),
        })}
      </p>

      {/* A window that has not been read yet is not a window of unknown size —
          don't render either claim until the probes settle. */}
      {ctx.inspectionsPending ? (
        <div className="space-y-2" role="status" aria-live="polite">
          <RowSkeleton />
          <RowSkeleton />
        </div>
      ) : (
        <ul className="space-y-2">
          {ctx.installedModels.map((model) => {
            const fit = assessModelContextFit({
              model,
              contextLength: ctx.inspections[model]?.contextLength,
            });
            const stageNames = fit.overflowStages
              .map((stage) => t(`settings.ai.stages.names.${stage}`))
              .join(', ');

            return (
              <li
                key={model}
                className="rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2"
              >
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className="text-xs font-medium text-foreground/80">{model}</span>
                  <span className={`text-[11px] ${VERDICT_CLASS[fit.verdict]}`}>
                    {t(`settings.ai.advisor.fit.verdict.${fit.verdict}`)}
                  </span>
                </div>

                {fit.verdict === 'unknown' ? (
                  <p className="mt-1 text-[11px] text-foreground/60">
                    {t('settings.ai.advisor.fit.unknownHint')}
                  </p>
                ) : (
                  <p className="mt-1 text-[11px] text-foreground/60">
                    {t('settings.ai.advisor.fit.usable', {
                      usable: (fit.usableInputTokens ?? 0).toLocaleString(i18n.language),
                      window: (fit.contextLength ?? 0).toLocaleString(i18n.language),
                    })}
                  </p>
                )}

                {/* Name the stage — "some stage won't fit" is not actionable. */}
                {fit.overflowStages.length > 0 && (
                  <p className="mt-1 text-[11px] text-amber-400">
                    {t('settings.ai.advisor.fit.overflow', { stages: stageNames })}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      )}

      <p className="text-[11px] leading-relaxed text-foreground/60">
        {t('settings.ai.advisor.fit.truncationNote')}
      </p>
    </div>
  );
}
