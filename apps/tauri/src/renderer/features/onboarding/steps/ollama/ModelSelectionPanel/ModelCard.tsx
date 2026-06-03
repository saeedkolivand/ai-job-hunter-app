import type { MODEL_RECS } from '@ajh/shared';
import { Button } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

type ModelRec = (typeof MODEL_RECS)[number];

interface Props {
  rec: ModelRec;
  selected: boolean;
  recommended: boolean;
  installed: boolean;
  tooHeavy: boolean;
  mightLagRam: boolean;
  mightLagVram: boolean;
  onSelect: () => void;
}

export function ModelCard({
  rec,
  selected,
  recommended,
  installed,
  tooHeavy,
  mightLagRam,
  mightLagVram,
  onSelect,
}: Props) {
  const { t } = useTranslation();

  return (
    <Button
      variant="unstyled"
      onClick={() => !tooHeavy && onSelect()}
      disabled={tooHeavy}
      className={`group relative w-full rounded-xl border px-4 py-3 text-left transition-all duration-150 ${
        selected
          ? 'border-brand/40 bg-brand/10'
          : tooHeavy
            ? 'border-white/[0.04] bg-white/[0.01] opacity-40 cursor-not-allowed'
            : 'border-white/[0.07] bg-white/[0.02] hover:border-white/20 hover:bg-white/[0.04]'
      }`}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span
              className={`text-sm font-medium ${
                selected ? 'text-foreground/90' : 'text-foreground/70'
              }`}
            >
              {rec.label}
            </span>
            {recommended && (
              <span className="rounded-full border border-brand/30 bg-brand/15 px-1.5 py-0.5 text-[10px] font-medium text-brand-soft">
                {t('onboarding.ai.recommended')}
              </span>
            )}
            {installed && (
              <span className="rounded-full border border-emerald-400/30 bg-emerald-400/10 px-1.5 py-0.5 text-[10px] font-medium text-emerald-300">
                {t('onboarding.ai.installed')}
              </span>
            )}
            {mightLagRam && (
              <span className="rounded-full border border-amber-400/30 bg-amber-400/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-300">
                {t('onboarding.ai.mayLagRam')}
              </span>
            )}
            {mightLagVram && (
              <span className="rounded-full border border-orange-400/30 bg-orange-400/10 px-1.5 py-0.5 text-[10px] font-medium text-orange-300">
                {t('onboarding.ai.mayLagVram')}
              </span>
            )}
          </div>
          <p className="mt-0.5 text-xs text-foreground/35">{rec.description}</p>
        </div>
        <div className="shrink-0 text-right">
          <span className="text-xs text-foreground/30">{rec.sizeGb} GB</span>
        </div>
      </div>
    </Button>
  );
}
