import { useTranslation } from '@ajh/translations';

import {
  assessModelContextFit,
  type ContextFitVerdict,
  estimateTokensFromChars,
  STAGE_WORST_CASE_CHARS,
} from './context-fit';
import type { AdvisorStepProps } from './types';

/** One token class per verdict — no `[#RRGGBB]`, and each state has its own
 *  visible treatment rather than only differing by wording. */
const VERDICT_CLASS: Record<ContextFitVerdict, string> = {
  fits: 'text-emerald-400/80',
  tight: 'text-amber-400/80',
  'too-small': 'text-red-400/80',
  unknown: 'text-foreground/40',
};

/**
 * Step 2 — does each model's window hold what the stages actually send?
 *
 * The demand side is the Rust prompt caps (see `context-fit.ts`), so the
 * verdict is a comparison against real bounds, not a rule of thumb. A model
 * with no measured window reads "not measured": the app never assumes a size.
 */
export function ContextFitStep({ ctx }: AdvisorStepProps) {
  const { t } = useTranslation();

  const draftTokens = estimateTokensFromChars(STAGE_WORST_CASE_CHARS.draft ?? 0);

  return (
    <div className="space-y-3">
      <p className="text-xs leading-relaxed text-foreground/55">
        {t('settings.ai.advisor.fit.intro', {
          chars: (STAGE_WORST_CASE_CHARS.draft ?? 0).toLocaleString(),
          tokens: draftTokens.toLocaleString(),
        })}
      </p>

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
                <p className="mt-1 text-[11px] text-foreground/40">
                  {t('settings.ai.advisor.fit.unknownHint')}
                </p>
              ) : (
                <p className="mt-1 text-[11px] text-foreground/45">
                  {t('settings.ai.advisor.fit.usable', {
                    usable: (fit.usableInputTokens ?? 0).toLocaleString(),
                    window: (fit.contextLength ?? 0).toLocaleString(),
                  })}
                </p>
              )}

              {/* Name the stage — "some stage won't fit" is not actionable. */}
              {fit.overflowStages.length > 0 && (
                <p className="mt-1 text-[11px] text-amber-400/80">
                  {t('settings.ai.advisor.fit.overflow', { stages: stageNames })}
                </p>
              )}
            </li>
          );
        })}
      </ul>

      <p className="text-[11px] leading-relaxed text-foreground/40">
        {t('settings.ai.advisor.fit.truncationNote')}
      </p>
    </div>
  );
}
