import { Building2, Globe, Home, type LucideIcon } from 'lucide-react';

import { GlassCard, OptionTile, SectionLabel } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useJobPreferences, useSetJobPreferences } from '@/services';

const REMOTE_OPTIONS: {
  value: string;
  labelKey: string;
  descriptionKey: string;
  icon: LucideIcon;
}[] = [
  {
    value: 'remote',
    labelKey: 'settings.remote.remote',
    descriptionKey: 'settings.remote.remoteDescription',
    icon: Home,
  },
  {
    value: 'hybrid',
    labelKey: 'settings.remote.hybrid',
    descriptionKey: 'settings.remote.hybridDescription',
    icon: Building2,
  },
  {
    value: 'on-site',
    labelKey: 'settings.remote.onSite',
    descriptionKey: 'settings.remote.onSiteDescription',
    icon: Building2,
  },
  {
    value: 'any',
    labelKey: 'settings.remote.any',
    descriptionKey: 'settings.remote.anyDescription',
    icon: Globe,
  },
];

export function RemotePreferences() {
  const { t } = useTranslation();
  const { data: jobPrefs } = useJobPreferences();
  const setJobPreferences = useSetJobPreferences();

  return (
    <GlassCard>
      <div className="mb-4">
        <SectionLabel>{t('settings.remote.title')}</SectionLabel>
      </div>
      <p className="mb-4 text-sm text-foreground/55">{t('settings.remote.description')}</p>
      <div className="grid grid-cols-2 gap-3">
        {REMOTE_OPTIONS.map((opt) => (
          <OptionTile
            key={opt.value}
            icon={opt.icon}
            label={t(opt.labelKey)}
            description={t(opt.descriptionKey)}
            selected={jobPrefs?.remote === opt.value}
            onClick={() => setJobPreferences.mutate({ ...jobPrefs, remote: opt.value })}
            layoutId="remote-selection"
          />
        ))}
      </div>
    </GlassCard>
  );
}
