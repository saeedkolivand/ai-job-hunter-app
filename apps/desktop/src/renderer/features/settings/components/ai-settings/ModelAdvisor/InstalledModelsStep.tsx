import { Cpu } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { EmptyState, RowSkeleton } from '@ajh/ui';

import type { AdvisorStepProps } from './types';

/**
 * Step 1 — what is actually installed, and what Ollama says about each model.
 *
 * `/api/tags` returns names only, so the size/quantization/window come from a
 * per-model `/api/show`. A model Ollama cannot describe is shown as NOT
 * MEASURED rather than being dropped from the list or given a default window.
 */
export function InstalledModelsStep({ ctx }: AdvisorStepProps) {
  const { t } = useTranslation();

  if (ctx.installedModels.length === 0) {
    return (
      <EmptyState
        icon={Cpu}
        title={t('settings.ai.advisor.models.emptyTitle')}
        description={t('settings.ai.advisor.models.emptyDescription')}
      />
    );
  }

  return (
    <div className="space-y-3">
      <p className="text-xs leading-relaxed text-foreground/55">
        {t('settings.ai.advisor.models.intro')}
      </p>

      {ctx.inspectionsPending && (
        <div className="space-y-2" role="status" aria-live="polite">
          <RowSkeleton />
          <RowSkeleton />
        </div>
      )}

      <ul className="space-y-2">
        {ctx.installedModels.map((model) => {
          const info = ctx.inspections[model];
          const facts = [
            info?.parameterSize,
            info?.quantization,
            info?.contextLength
              ? t('settings.ai.advisor.models.window', {
                  tokens: info.contextLength.toLocaleString(),
                })
              : undefined,
          ].filter(Boolean);

          return (
            <li
              key={model}
              className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2"
            >
              <span className="text-xs font-medium text-foreground/80">
                {model}
                {model === ctx.activeModel && (
                  <span className="ml-2 rounded-full border border-brand/30 bg-brand/10 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-brand-soft">
                    {t('settings.ai.advisor.models.active')}
                  </span>
                )}
              </span>
              <span className="text-[11px] text-foreground/45">
                {facts.length > 0 ? facts.join(' · ') : t('settings.ai.advisor.models.notMeasured')}
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
