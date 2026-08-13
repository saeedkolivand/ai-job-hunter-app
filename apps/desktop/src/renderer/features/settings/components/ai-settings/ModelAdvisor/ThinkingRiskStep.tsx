import { AlertTriangle, Gauge } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { EmptyState } from '@ajh/ui';

import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';

import { assessThinkingRisk, type ThinkingRiskLevel } from './thinking-risk';
import type { AdvisorStepProps } from './types';

const LEVEL_CLASS: Record<ThinkingRiskLevel, string> = {
  'known-bad-combo': 'text-red-400',
  'measured-heavy': 'text-amber-400',
  'reasoning-heavy': 'text-amber-400',
  'measured-light': 'text-emerald-400',
  unmeasured: 'text-foreground/60',
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

  // Keyed by PROVIDER + model, because that pair is what an assessment is about:
  // a measurement counts only when both match. Keying by model alone let an
  // override that names the active model on a different provider overwrite the
  // provider for that key — one row silently dropped, and the survivor assessed
  // against the wrong provider.
  const seen = new Map<string, { model: string; provider?: string }>();
  const add = (model: string, provider?: string) =>
    seen.set(`${provider ?? ''}::${model}`, { model, provider });
  if (ctx.activeModel) add(ctx.activeModel, ctx.activeProvider);
  for (const override of Object.values(ctx.overrides)) {
    if (override.model) add(override.model, override.provider);
  }
  const models = [...seen.entries()];

  // Nothing selected anywhere (no active model, no overrides) — say that,
  // rather than showing intro prose above an empty list.
  if (models.length === 0) {
    return (
      <EmptyState
        icon={Gauge}
        title={t('settings.ai.advisor.risk.emptyTitle')}
        description={t('settings.ai.advisor.risk.emptyDescription')}
      />
    );
  }

  return (
    <div className="space-y-3">
      <p className="text-xs leading-relaxed text-foreground/60">
        {t('settings.ai.advisor.risk.intro')}
      </p>

      <ul className="space-y-2">
        {models.map(([rowKey, { model, provider }]) => {
          const risk = assessThinkingRisk({
            model,
            provider,
            effort: ctx.effort,
            measured: ctx.thinkingByModel,
          });
          const alarming = risk.level === 'known-bad-combo' || risk.level === 'measured-heavy';

          return (
            <li
              key={rowKey}
              className="rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="text-xs font-medium text-foreground/80">
                  {model}
                  {provider && provider !== ctx.activeProvider && (
                    <span className="ml-1.5 text-foreground/60">
                      ({PROVIDERS[provider as AiProvider]?.label ?? provider})
                    </span>
                  )}
                </span>
                <span className={`flex items-center gap-1 text-[11px] ${LEVEL_CLASS[risk.level]}`}>
                  {alarming && <AlertTriangle size={11} aria-hidden="true" />}
                  {t(`settings.ai.advisor.risk.level.${risk.level}`)}
                </span>
              </div>

              {risk.level === 'known-bad-combo' && (
                <p className="mt-1 text-[11px] leading-relaxed text-foreground/60">
                  {t('settings.ai.advisor.risk.badCombo', { effort: risk.effort })}
                </p>
              )}
              {risk.level === 'measured-heavy' &&
                (risk.ratio === undefined ? (
                  // Thinking tokens with zero output: no ratio exists to quote.
                  <p className="mt-1 text-[11px] leading-relaxed text-foreground/60">
                    {t('settings.ai.advisor.risk.measuredNoOutput', { calls: risk.calls ?? 0 })}
                  </p>
                ) : (
                  <p className="mt-1 text-[11px] leading-relaxed text-foreground/60">
                    {t('settings.ai.advisor.risk.measured', {
                      ratio: risk.ratio.toFixed(1),
                      calls: risk.calls ?? 0,
                    })}
                  </p>
                ))}
              {risk.level === 'measured-light' && (
                <p className="mt-1 text-[11px] leading-relaxed text-foreground/60">
                  {t('settings.ai.advisor.risk.measuredLight', {
                    ratio: (risk.ratio ?? 0).toFixed(1),
                    calls: risk.calls ?? 0,
                  })}
                </p>
              )}
              {risk.level === 'reasoning-heavy' && (
                <p className="mt-1 text-[11px] leading-relaxed text-foreground/60">
                  {t('settings.ai.advisor.risk.reasoningHeavy')}
                </p>
              )}
              {risk.level === 'unmeasured' && (
                <p className="mt-1 text-[11px] leading-relaxed text-foreground/60">
                  {t('settings.ai.advisor.risk.unmeasured')}
                </p>
              )}
            </li>
          );
        })}
      </ul>

      <p className="text-[11px] leading-relaxed text-foreground/60">
        {t('settings.ai.advisor.risk.coverageNote')}
      </p>
    </div>
  );
}
