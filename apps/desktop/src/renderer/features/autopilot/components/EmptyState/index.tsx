import { Calendar, Filter, Plus, Send, Target, Zap } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

interface EmptyStateProps {
  onNew(): void;
}

export function EmptyState({ onNew }: EmptyStateProps) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center gap-6 py-20 text-center">
      <div className="flex h-20 w-20 items-center justify-center rounded-2xl bg-brand/10 ring-1 ring-brand/20">
        <Zap size={36} className="text-brand-soft/60" />
      </div>
      <div>
        <div className="text-lg font-semibold text-foreground/50">{t('autopilot.empty.title')}</div>
        <div className="mt-1 text-sm text-foreground/30 max-w-sm">
          {t('autopilot.empty.description')}
        </div>
      </div>
      <div className="flex flex-col gap-2 text-left">
        {[
          { icon: Target, text: t('autopilot.empty.step1') },
          { icon: Filter, text: t('autopilot.empty.step2') },
          { icon: Send, text: t('autopilot.empty.step3') },
          { icon: Calendar, text: t('autopilot.empty.step4') },
        ].map(({ icon: Icon, text }, i) => (
          <div key={i} className="flex items-center gap-2.5 text-xs text-foreground/35">
            <Icon size={12} className="text-brand-soft/50 shrink-0" /> {text}
          </div>
        ))}
      </div>
      <Button
        variant="primary"
        size="md"
        onClick={onNew}
        className="transition-all duration-150 ease-out px-6 gap-2"
      >
        <Plus size={14} /> {t('autopilot.empty.createFirst')}
      </Button>
    </div>
  );
}
