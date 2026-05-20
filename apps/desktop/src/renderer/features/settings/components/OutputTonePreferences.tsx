import { type LucideIcon, MessageSquare, Briefcase, FileText, Sparkles } from 'lucide-react';
import { GlassCard } from '@/components/ui/GlassCard';
import { SectionLabel } from '@/components/ui/SectionLabel';
import { OptionTile } from '@/components/ui/OptionTile';
import { usePreferencesStore, useOutputTone } from '@/store/preferences-store';
import type { OutputTone } from '@/store/preferences-schema';
import { useTranslation } from '@/lib/i18n';

const TONE_OPTIONS: {
  value: OutputTone;
  labelKey: string;
  descriptionKey: string;
  icon: LucideIcon;
}[] = [
  {
    value: 'professional',
    labelKey: 'settings.outputTone.professional',
    descriptionKey: 'settings.outputTone.professionalDescription',
    icon: Briefcase,
  },
  {
    value: 'casual',
    labelKey: 'settings.outputTone.casual',
    descriptionKey: 'settings.outputTone.casualDescription',
    icon: MessageSquare,
  },
  {
    value: 'formal',
    labelKey: 'settings.outputTone.formal',
    descriptionKey: 'settings.outputTone.formalDescription',
    icon: FileText,
  },
  {
    value: 'creative',
    labelKey: 'settings.outputTone.creative',
    descriptionKey: 'settings.outputTone.creativeDescription',
    icon: Sparkles,
  },
];

export function OutputTonePreferences() {
  const { t } = useTranslation();
  const outputTone = useOutputTone();
  const setOutputTone = usePreferencesStore((s) => s.setOutputTone);

  return (
    <GlassCard>
      <div className="mb-4">
        <SectionLabel>{t('settings.outputTone.title')}</SectionLabel>
      </div>
      <p className="mb-4 text-sm text-foreground/55">{t('settings.outputTone.description')}</p>
      <div className="grid grid-cols-2 gap-3">
        {TONE_OPTIONS.map((opt) => (
          <OptionTile
            key={opt.value}
            icon={opt.icon}
            label={t(opt.labelKey)}
            description={t(opt.descriptionKey)}
            selected={outputTone === opt.value}
            onClick={() => setOutputTone(opt.value)}
            layoutId="tone-selection"
          />
        ))}
      </div>
    </GlassCard>
  );
}
