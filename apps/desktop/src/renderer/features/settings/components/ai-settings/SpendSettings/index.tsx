import { Coins } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { EmptyState, ErrorState, RowSkeleton, SettingsSection } from '@ajh/ui';

import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useSpendSummary } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';

/** "$0" for zero, "<$0.01" for a sub-cent estimate, else "~$X.XX". Numbers/symbols
 *  only — no locale-currency formatting (over-engineering for a rough estimate). */
function formatEstCost(usd: number): string {
  if (usd <= 0) return '$0';
  if (usd < 0.01) return '<$0.01';
  return `~$${usd.toFixed(2)}`;
}

export function SpendSettings() {
  const { t } = useTranslation();
  const { data, isLoading, isError, refetch } = useSpendSummary();

  const isEmpty = !isLoading && !isError && (!data || data.perProvider.length === 0);

  return (
    <SettingsSection icon={Coins} label={t('settings.spend.heading')}>
      <p className="mb-3 text-xs leading-relaxed text-foreground/50">
        {t('settings.spend.disclaimer')}
      </p>

      {isLoading && (
        <div className="space-y-2">
          <RowSkeleton />
          <RowSkeleton />
        </div>
      )}

      {isError && (
        <ErrorState
          title={t('settings.spend.errorTitle')}
          description={t('settings.spend.errorDescription')}
          onRetry={() => void refetch()}
        />
      )}

      {isEmpty && <EmptyState icon={Coins} title={t('settings.spend.emptyTitle')} />}

      {!isLoading && !isError && !isEmpty && data && (
        <div className="space-y-3">
          <div className="flex items-center justify-between rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2.5">
            <span className="text-sm font-semibold text-foreground/80">
              {formatEstCost(data.today.estCostUsd)}
            </span>
            <span className="text-[11px] text-foreground/40">
              {t('settings.spend.tokensSummary', {
                input: data.today.inputTokens.toLocaleString(),
                output: data.today.outputTokens.toLocaleString(),
              })}
            </span>
          </div>

          <div className="space-y-1.5">
            <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
              {t('settings.spend.perProviderHeading')}
            </div>
            {data.perProvider.map((p) => {
              const label = PROVIDERS[p.provider as AiProvider]?.label ?? p.provider;
              return (
                <div
                  key={p.provider}
                  className="flex items-center justify-between gap-2 rounded-lg bg-foreground/[0.03] px-3 py-2 text-xs"
                >
                  <span className="text-foreground/70">{label}</span>
                  <span className="text-foreground/40">
                    {t('settings.spend.tokensSummary', {
                      input: p.inputTokens.toLocaleString(),
                      output: p.outputTokens.toLocaleString(),
                    })}
                  </span>
                  <span className="font-medium text-foreground/70">
                    {p.estCostUsd <= 0
                      ? t('settings.spend.freeLocal')
                      : formatEstCost(p.estCostUsd)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </SettingsSection>
  );
}
