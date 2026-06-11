import { MemoryStick } from 'lucide-react';

import { useTranslation } from '@ajh/translations';

interface Props {
  totalRamGb: number;
  freeRamGb: number;
  hasGpu: boolean;
  freeVramGb: number;
  totalVramGb: number;
  usedVramGb: number;
  cpuCount?: number;
  deviceTier: { label: string; color: string };
}

export function HardwarePopover({
  totalRamGb,
  freeRamGb,
  hasGpu,
  freeVramGb,
  totalVramGb,
  usedVramGb,
  cpuCount,
  deviceTier,
}: Props) {
  const { t } = useTranslation();

  return (
    <div className="relative group">
      <div className="flex items-center gap-3 rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3 cursor-help">
        <MemoryStick size={14} className="text-foreground/30" />
        <div className="flex-1">
          <span className="text-xs text-foreground/40">{t('onboarding.ai.systemPerformance')}</span>
        </div>
        <span className={`text-xs font-medium ${deviceTier.color}`}>{deviceTier.label}</span>
      </div>
      {/* Popover with detailed info */}
      <div className="absolute left-0 top-full z-50 mt-2 w-64 rounded-xl border border-white/[0.1] bg-black/95 p-4 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-200 shadow-2xl">
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs text-foreground/50">{t('onboarding.ai.ramLabel')}</span>
            <span className="text-xs font-medium text-foreground/90">
              {totalRamGb} GB ({freeRamGb} GB {t('onboarding.ai.free')})
            </span>
          </div>
          {cpuCount && (
            <div className="flex items-center justify-between">
              <span className="text-xs text-foreground/50">{t('onboarding.ai.cpuLabel')}</span>
              <span className="text-xs font-medium text-foreground/90">
                {cpuCount} {t('onboarding.ai.cores')}
              </span>
            </div>
          )}
          {hasGpu && (
            <div className="flex items-center justify-between">
              <span className="text-xs text-foreground/50">{t('onboarding.ai.vramLabel')}</span>
              <span className="text-xs font-medium text-foreground/90">
                {usedVramGb} / {totalVramGb} GB ({freeVramGb} GB {t('onboarding.ai.free')})
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
