import { Bot, CheckCircle2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, GlassCard } from '@ajh/ui';

import type { AiProvider } from '@/store/preferences-schema';

interface ProviderMeta {
  label: string;
  color: string;
}

interface Props {
  providers: AiProvider[];
  meta: Record<AiProvider, ProviderMeta>;
  activeProvider: AiProvider;
  onSetActive: (provider: AiProvider) => void;
}

export function ActiveProviderSwitcher({ providers, meta, activeProvider, onSetActive }: Props) {
  const { t } = useTranslation();

  if (providers.length <= 1) return null;

  return (
    <GlassCard>
      <div className="mb-3 text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
        {t('settings.aiProvider.activeProvider')}
      </div>
      <div className="flex flex-wrap gap-1.5">
        {providers.map((p) => {
          const m = meta[p];
          const isActive = p === activeProvider;
          return (
            <Button
              key={p}
              variant="unstyled"
              onClick={() => onSetActive(p)}
              className={`flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-medium transition-all ${
                isActive
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-foreground/10 bg-foreground/[0.03] text-foreground/50 hover:border-foreground/20 hover:text-foreground/80'
              }`}
            >
              <Bot size={11} className={isActive ? m.color : ''} />
              {m.label}
              {isActive && <CheckCircle2 size={10} className="text-brand-soft" />}
            </Button>
          );
        })}
      </div>
    </GlassCard>
  );
}
