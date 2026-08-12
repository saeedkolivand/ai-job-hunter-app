import { AlertTriangle } from 'lucide-react';

import { useTranslation } from '@ajh/translations';

import { assessThinkingRisk, type ThinkingRiskLevel } from './thinking-risk';
import type { AdvisorStepProps } from './types';

const LEVEL_CLASS: Record<ThinkingRiskLevel, string> = {
  'known-bad-combo': 'text-red-400/80',
  'measured-heavy': 'text-amber-400/80',
  'reasoning-heavy': 'text-amber-400/70',
  unmeasured: 'text-foreground/45',
};

/**
 * Step 4 — the combinations that are known to go wrong.
 *
 * Every model in play is assessed (the active one plus anything a stage
 * override names), against BOTH signals: the provider's reported reasoning
 * split where it exists, and the model-name heuristic — which is the only
 * signal that exists for a local-only user, since Ollama folds thinking into
 * its output count and therefore never appears in the measured list.
 *
 * "Not measured" is rendered as its own state, never as reassurance.
 */
export function ThinkingRiskStep({ ctx }: AdvisorStepProps) {
  const { t } = useTranslation();

  const models = [
    ...new Set(
      [ctx.activeModel, ...Object.values(ctx.overrides).map((o) => o.model)].filter(
        (m): m is string => Boolean(m)
      )
    ),
  ];

  return (
    <div className="space-y-3">
      <p className="text-xs leading-relaxed text-foreground/55">
        {t('settings.ai.advisor.risk.intro')}
      </p>

      <ul className="space-y-2">
        {models.map((model) => {
          const risk = assessThinkingRisk({
            model,
            effort: ctx.effort,
            measured: ctx.thinkingByModel,
          });
          const alarming = risk.level === 'known-bad-combo' || risk.level === 'measured-heavy';

          return (
            <li
              key={model}
              className="rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="text-xs font-medium text-foreground/80">{model}</span>
                <span className={`flex items-center gap-1 text-[11px] ${LEVEL_CLASS[risk.level]}`}>
                  {alarming && <AlertTriangle size={11} aria-hidden="true" />}
                  {t(`settings.ai.advisor.risk.level.${risk.level}`)}
                </span>
              </div>

              {risk.level === 'known-bad-combo' && (
                <p className="mt-1 text-[11px] leading-relaxed text-foreground/55">
                  {t('settings.ai.advisor.risk.badCombo', { effort: risk.effort })}
                </p>
              )}
              {risk.level === 'measured-heavy' && (
                <p className="mt-1 text-[11px] leading-relaxed text-foreground/55">
                  {t('settings.ai.advisor.risk.measured', {
                    ratio: (risk.ratio ?? 0).toFixed(1),
                    calls: risk.calls ?? 0,
                  })}
                </p>
              )}
              {risk.level === 'reasoning-heavy' && (
                <p className="mt-1 text-[11px] leading-relaxed text-foreground/55">
                  {t('settings.ai.advisor.risk.reasoningHeavy')}
                </p>
              )}
              {risk.level === 'unmeasured' && (
                <p className="mt-1 text-[11px] leading-relaxed text-foreground/45">
                  {t('settings.ai.advisor.risk.unmeasured')}
                </p>
              )}
            </li>
          );
        })}
      </ul>

      <p className="text-[11px] leading-relaxed text-foreground/40">
        {t('settings.ai.advisor.risk.coverageNote')}
      </p>
    </div>
  );
}
