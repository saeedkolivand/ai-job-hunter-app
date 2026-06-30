import { Gift, Heart, Wallet } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, SectionLabel } from '@ajh/ui';

import { useAppVersion, useOpenExternal } from '@/services';

const DONATE_LINKS = [
  {
    key: 'github' as const,
    href: 'https://github.com/sponsors/saeedkolivand',
    icon: Gift,
    labelKey: 'settings.about.githubSponsors',
  },
  {
    key: 'kofi' as const,
    href: 'https://ko-fi.com/saeedkolivand',
    icon: Heart,
    labelKey: 'settings.about.kofi',
  },
  {
    key: 'paypal' as const,
    href: 'https://paypal.me/saeedkolivand',
    icon: Wallet,
    labelKey: 'settings.about.paypal',
  },
];

export function AboutTab() {
  const { t } = useTranslation();
  const { data: versionRaw = '' } = useAppVersion();
  const version = versionRaw
    ? String(versionRaw).startsWith('v')
      ? String(versionRaw)
      : `v${versionRaw}`
    : '';
  const openExternal = useOpenExternal();

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <Heart size={15} className="text-brand-soft" aria-hidden="true" />
        <SectionLabel>{t('settings.about.title')}</SectionLabel>
      </div>

      <div className="space-y-4">
        {version && <p className="font-mono text-xs text-foreground/40">{version}</p>}

        <p className="text-sm text-foreground/60">{t('settings.about.pitch')}</p>

        <div className="space-y-2 pt-1">
          {DONATE_LINKS.map(({ key, href, icon: Icon, labelKey }) => (
            <Button
              key={key}
              variant="glass"
              className="w-full justify-start gap-2 text-xs"
              onClick={() => openExternal.mutate(href)}
            >
              <Icon size={13} />
              {t(labelKey)}
            </Button>
          ))}
        </div>
      </div>
    </GlassCard>
  );
}
