import { MemoryStick } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, HoverPopover } from '@ajh/ui';

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
    <HoverPopover
      placement="bottom"
      ariaLabel={t('onboarding.ai.systemPerformance')}
      contentClassName="w-64 rounded-xl border border-[var(--border-clear)] bg-card p-4 shadow-2xl"
      trigger={
        <Button
          variant="unstyled"
          aria-label={t('onboarding.ai.systemPerformance')}
          className="flex w-full items-center gap-3 rounded-xl border border-[var(--border-clear)] bg-card px-4 py-3 cursor-help text-left"
        >
          <MemoryStick size={14} className="text-foreground/30" />
          <div className="flex-1">
            <span className="text-xs text-foreground/40">
              {t('onboarding.ai.systemPerformance')}
            </span>
          </div>
          <span className={`text-xs font-medium ${deviceTier.color}`}>{deviceTier.label}</span>
        </Button>
      }
    >
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
    </HoverPopover>
  );
}
