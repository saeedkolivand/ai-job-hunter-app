import { Download, LogOut, RotateCcw, Trash2, Upload } from 'lucide-react';
import { useState } from 'react';

import { ConfirmModal, useNotification } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import {
  useClearInteractions,
  useExportData,
  useImportData,
  useResetApp,
  useSignOutAll,
} from '@/services';

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
      notify(t('settings.privacy.signedOutSuccess'), 'success');
    } catch {
      notify(t('settings.privacy.somethingWentWrong'), 'error');
    }
  };

  const handleClearInteractions = async () => {
    setConfirm((c) => ({ ...c, open: false }));
    try {
      await clearInteractions.mutateAsync();
      notify(t('settings.privacy.historyClearedSuccess'), 'success');
    } catch {
      notify(t('settings.privacy.somethingWentWrong'), 'error');
    }
  };

  const handleResetApp = async () => {
    setConfirm((c) => ({ ...c, open: false }));
    try {
      await resetApp.mutateAsync();
      // resetPreferences() is called inside useResetApp onSuccess,
      // which sets onboardingCompleted: false — the wizard re-mounts at welcome step
    } catch {
      notify(t('settings.privacy.somethingWentWrong'), 'error');
    }
  };

  const handleExport = async () => {
    try {
      const res = (await exportData.mutateAsync()) as { success: boolean; error?: string };
      if (res.success) notify(t('settings.privacy.exportSuccess'), 'success');
      else if (res.error) notify(t('settings.privacy.somethingWentWrong'), 'error');
    } catch {
      notify(t('settings.privacy.somethingWentWrong'), 'error');
    }
  };

  const handleImport = async () => {
    try {
      const res = (await importData.mutateAsync()) as {
        success: boolean;
        imported?: Record<string, number | { error: string }>;
        error?: string;
      };
      if (res.success) {
        const count = Object.values(res.imported ?? {}).reduce<number>(
          (sum, v) => sum + (typeof v === 'number' ? v : 0),
          0
        );
        notify(
          t('settings.privacy.importSuccess', { count, plural: count === 1 ? '' : 's' }),
          'success'
        );
      } else if (res.error) notify(t('settings.privacy.somethingWentWrong'), 'error');
    } catch {
      notify(t('settings.privacy.somethingWentWrong'), 'error');
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
      {cards.map(({ key, ...card }) => (
        <ActionCard key={key} {...card} />
      ))}

      {/* ── Danger Zone ─────────────────────────────────────────────── */}
      <div className="mt-2 rounded-xl border border-rose-700/40 bg-rose-950/20 p-3">
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
