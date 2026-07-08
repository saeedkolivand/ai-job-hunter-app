import { Bug, Gift, Heart, Wallet } from 'lucide-react';
import { save } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';

import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, SectionLabel, useNotification } from '@ajh/ui';

import { useAppVersion, useExportDiagnostics, useOpenExternal } from '@/services';

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
  const notify = useNotification();
  const exportDiagnostics = useExportDiagnostics();

  const handleExportDiagnostics = async () => {
    const now = new Date();
    const stamp = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(
      now.getDate()
    ).padStart(2, '0')}`;
    try {
      const dest = await save({
        defaultPath: `ajh-diagnostics-${stamp}.zip`,
        filters: [{ name: 'Zip archive', extensions: ['zip'] }],
      });
      if (!dest) return;
      const res = await exportDiagnostics.mutateAsync(dest);
      if (res.success) {
        notify.success({ message: t('settings.about.exportDiagnosticsSaved') });
        revealItemInDir(dest).catch(() => {});
      } else {
        notify.error({ message: t('settings.about.exportDiagnosticsError') });
      }
    } catch (err) {
      console.error('diagnostics export failed:', err instanceof Error ? err.name : 'unknown');
      notify.error({ message: t('settings.about.exportDiagnosticsError') });
    }
  };

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

        <div className="space-y-2 pt-1">
          <p className="text-xs text-foreground/40">{t('settings.about.exportDiagnosticsDesc')}</p>
          <Button
            variant="glass"
            className="w-full justify-start gap-2 text-xs"
            loading={exportDiagnostics.isPending}
            onClick={handleExportDiagnostics}
          >
            <Bug size={13} />
            {t('settings.about.exportDiagnostics')}
          </Button>
        </div>
      </div>
    </GlassCard>
  );
}
