import { Download, LogOut, RotateCcw, Shield, Trash2, Upload } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { ConfirmModal, SettingsSection, Switch, useNotification } from '@ajh/ui';

import {
  useClearInteractions,
  useExportData,
  useImportData,
  useResetApp,
  useSignOutAll,
} from '@/services';
import { useFetchCompanyLogos, usePreferencesStore } from '@/store/preferences-store';

import { ActionCard, type ActionCardProps } from './ActionCard';

type ConfirmAction = 'signOut' | 'clearInteractions' | 'resetApp';

export function PrivacySettingsTab() {
  const { t } = useTranslation();
  const [confirm, setConfirm] = useState<{ open: boolean; action: ConfirmAction }>({
    open: false,
    action: 'signOut',
  });
  const notify = useNotification();

  const signOutAll = useSignOutAll();
  const clearInteractions = useClearInteractions();
  const exportData = useExportData();
  const importData = useImportData();
  const resetApp = useResetApp();

  const fetchCompanyLogos = useFetchCompanyLogos();
  const setFetchCompanyLogos = usePreferencesStore((s) => s.setFetchCompanyLogos);

  const busy: Partial<Record<ConfirmAction | 'export' | 'import', boolean>> = {
    signOut: signOutAll.isPending,
    clearInteractions: clearInteractions.isPending,
    resetApp: resetApp.isPending,
    export: exportData.isPending,
    import: importData.isPending,
  };

  const handleSignOut = async () => {
    setConfirm((c) => ({ ...c, open: false }));
    try {
      await signOutAll.mutateAsync();
      notify.success({ message: t('settings.privacy.signedOutSuccess') });
    } catch {
      notify.error({ message: t('settings.privacy.somethingWentWrong') });
    }
  };

  const handleClearInteractions = async () => {
    setConfirm((c) => ({ ...c, open: false }));
    try {
      await clearInteractions.mutateAsync();
      notify.success({ message: t('settings.privacy.historyClearedSuccess') });
    } catch {
      notify.error({ message: t('settings.privacy.somethingWentWrong') });
    }
  };

  const handleResetApp = async () => {
    setConfirm((c) => ({ ...c, open: false }));
    try {
      const res = await resetApp.mutateAsync();
      // resetPreferences() is called inside useResetApp onSuccess,
      // which sets onboardingCompleted: false — the wizard re-mounts at welcome step.
      // A partial reset (success:false) still wiped the stores but left board login
      // sessions on disk — warn instead of silently reporting a clean reset.
      if (!res.success) notify.error({ message: t('settings.privacy.resetAppPartial') });
    } catch {
      notify.error({ message: t('settings.privacy.somethingWentWrong') });
    }
  };

  const handleExport = async () => {
    try {
      const res = (await exportData.mutateAsync()) as { success: boolean; error?: string };
      if (res.success) notify.success({ message: t('settings.privacy.exportSuccess') });
      else if (res.error) notify.error({ message: t('settings.privacy.somethingWentWrong') });
    } catch {
      notify.error({ message: t('settings.privacy.somethingWentWrong') });
    }
  };

  const handleImport = async () => {
    try {
      const res = await importData.mutateAsync();
      if (res.success) {
        const count = Object.values(res.imported ?? {}).reduce<number>(
          (sum, v) => sum + (typeof v === 'number' ? v : 0),
          0
        );
        notify.success({
          message: t('settings.privacy.importSuccess', { count, plural: count === 1 ? '' : 's' }),
        });
      } else if (res.partial) {
        notify.error({ message: t('settings.privacy.importPartial') });
      } else if (res.error) notify.error({ message: t('settings.privacy.somethingWentWrong') });
    } catch {
      notify.error({ message: t('settings.privacy.somethingWentWrong') });
    }
  };

  const CONFIRM_CONFIG: Record<
    ConfirmAction,
    {
      title: string;
      description: string;
      confirmText: string;
      variant: 'danger' | 'warning';
    }
  > = {
    signOut: {
      title: t('settings.privacy.signOutAllConfirmTitle'),
      description: t('settings.privacy.signOutAllConfirmDescription'),
      confirmText: t('settings.privacy.signOutAllConfirm'),
      variant: 'warning',
    },
    clearInteractions: {
      title: t('settings.privacy.clearHistoryConfirmTitle'),
      description: t('settings.privacy.clearHistoryConfirmDescription'),
      confirmText: t('settings.privacy.clearHistoryConfirm'),
      variant: 'danger',
    },
    resetApp: {
      title: t('settings.privacy.resetAppConfirmTitle'),
      description: t('settings.privacy.resetAppConfirmDescription'),
      confirmText: t('settings.privacy.resetAppConfirm'),
      variant: 'danger',
    },
  };

  const cfg = CONFIRM_CONFIG[confirm.action];

  const cards: (ActionCardProps & { key: string })[] = [
    {
      key: 'export',
      icon: Download,
      iconBg: 'bg-emerald-600',
      iconColor: 'text-white',
      glowColor: 'rgba(16,185,129,0.18)',
      title: t('settings.privacy.exportData'),
      description: t('settings.privacy.exportDataDescription'),
      buttonLabel: t('settings.privacy.export'),
      buttonBorder: 'border-emerald-500/50',
      buttonText: 'text-emerald-400',
      buttonGlow: '0 0 16px rgba(16,185,129,0.15)',
      loading: !!busy.export,
      onClick: () => void handleExport(),
    },
    {
      key: 'import',
      icon: Upload,
      iconBg: 'bg-blue-600',
      iconColor: 'text-white',
      glowColor: 'rgba(59,130,246,0.18)',
      title: t('settings.privacy.importData'),
      description: t('settings.privacy.importDataDescription'),
      buttonLabel: t('settings.privacy.import'),
      buttonBorder: 'border-blue-500/50',
      buttonText: 'text-blue-400',
      buttonGlow: '0 0 16px rgba(59,130,246,0.15)',
      loading: !!busy.import,
      onClick: () => void handleImport(),
    },
    {
      key: 'signOut',
      icon: LogOut,
      iconBg: 'bg-amber-600',
      iconColor: 'text-white',
      glowColor: 'rgba(245,158,11,0.18)',
      title: t('settings.privacy.signOutAll'),
      description: t('settings.privacy.signOutAllDescription'),
      buttonLabel: t('settings.privacy.signOut'),
      buttonBorder: 'border-amber-500/50',
      buttonText: 'text-amber-400',
      buttonGlow: '0 0 16px rgba(245,158,11,0.15)',
      loading: !!busy.signOut,
      onClick: () => setConfirm({ open: true, action: 'signOut' }),
    },
    {
      key: 'clearInteractions',
      icon: Trash2,
      iconBg: 'bg-red-600',
      iconColor: 'text-white',
      glowColor: 'rgba(239,68,68,0.18)',
      title: t('settings.privacy.clearHistory'),
      description: t('settings.privacy.clearHistoryDescription'),
      buttonLabel: t('settings.privacy.clear'),
      buttonBorder: 'border-red-500/50',
      buttonText: 'text-red-400',
      buttonGlow: '0 0 16px rgba(239,68,68,0.15)',
      loading: !!busy.clearInteractions,
      onClick: () => setConfirm({ open: true, action: 'clearInteractions' }),
    },
  ];

  return (
    <div className="space-y-3">
      {/* ── Enrichment ─────────────────────────────────────────────────── */}
      <div data-settings-anchor="privacy-enrichment">
        <SettingsSection icon={Shield} label={t('settings.privacy.fetchCompanyLogosTitle')}>
          <div className="flex items-start gap-4 rounded-xl border border-foreground/10 px-4 py-3.5">
            <div className="min-w-0 flex-1">
              <div className="text-sm font-semibold text-foreground/90">
                {t('settings.privacy.fetchCompanyLogosTitle')}
              </div>
              <div className="mt-0.5 text-xs leading-snug text-foreground/40">
                {t('settings.privacy.fetchCompanyLogosDescription')}
              </div>
            </div>
            <Switch
              checked={fetchCompanyLogos}
              onCheckedChange={setFetchCompanyLogos}
              aria-label={t('settings.privacy.fetchCompanyLogosTitle')}
            />
          </div>
        </SettingsSection>
      </div>

      <div data-settings-anchor="privacy-data">
        <SettingsSection icon={Shield} label={t('settings.privacy.dataTitle')}>
          <div className="space-y-3">
            {cards.map(({ key, ...card }) => (
              <ActionCard key={key} {...card} />
            ))}
          </div>
        </SettingsSection>
      </div>

      {/* ── Danger Zone ─────────────────────────────────────────────── */}
      <div
        data-settings-anchor="privacy-reset"
        className="mt-2 rounded-xl border border-red-500/30 bg-red-500/[0.08] p-3"
      >
        <div className="mb-2 flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-widest text-rose-500/80">
          <RotateCcw size={10} />
          {t('settings.privacy.dangerZone')}
        </div>
        <ActionCard
          icon={RotateCcw}
          iconBg="bg-rose-700"
          iconColor="text-white"
          glowColor="rgba(190,18,60,0.18)"
          title={t('settings.privacy.resetApp')}
          description={t('settings.privacy.resetAppDescription')}
          buttonLabel={t('settings.privacy.reset')}
          buttonBorder="border-rose-600/50"
          buttonText="text-rose-400"
          buttonGlow="0 0 16px rgba(190,18,60,0.15)"
          loading={!!busy.resetApp}
          onClick={() => setConfirm({ open: true, action: 'resetApp' })}
        />
      </div>

      <ConfirmModal
        open={confirm.open}
        onClose={() => setConfirm({ ...confirm, open: false })}
        onConfirm={() => {
          if (confirm.action === 'signOut') void handleSignOut();
          else if (confirm.action === 'clearInteractions') void handleClearInteractions();
          else void handleResetApp();
        }}
        title={cfg.title}
        description={cfg.description}
        confirmText={cfg.confirmText}
        variant={cfg.variant}
        isConfirming={!!busy[confirm.action]}
      />
    </div>
  );
}
