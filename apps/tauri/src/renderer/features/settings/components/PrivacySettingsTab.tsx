import { Download, LogOut, RotateCcw, Trash2, Upload } from 'lucide-react';
import { useState } from 'react';

import { Button, ConfirmModal, useNotification } from '@ajh/ui';

import { cn } from '@ajh/ui';
import { useTranslation } from '@/lib/i18n';
import {
  useClearInteractions,
  useExportData,
  useImportData,
  useResetApp,
  useSignOutAll,
} from '@/services';

type ConfirmAction = 'signOut' | 'clearInteractions' | 'resetApp';

interface ActionCardProps {
  icon: React.ElementType;
  iconBg: string;
  iconColor: string;
  glowColor: string;
  title: string;
  description: string;
  buttonLabel: string;
  buttonBorder: string;
  buttonText: string;
  buttonGlow: string;
  loading?: boolean;
  onClick: () => void;
}

function ActionCard({
  icon: Icon,
  iconBg,
  iconColor,
  glowColor,
  title,
  description,
  buttonLabel,
  buttonBorder,
  buttonText,
  buttonGlow,
  loading,
  onClick,
}: ActionCardProps) {
  return (
    <div
      className="relative flex items-center gap-4 overflow-hidden rounded-xl border border-white/[0.07] px-4 py-3.5"
      style={{
        background:
          'linear-gradient(135deg, rgba(255,255,255,0.03) 0%, rgba(255,255,255,0.01) 100%)',
      }}
    >
      {/* Ambient glow behind icon */}
      <div
        className="pointer-events-none absolute -bottom-4 -left-4 h-24 w-24 rounded-full blur-2xl"
        style={{ background: glowColor }}
      />

      {/* Icon */}
      <div
        className={cn(
          'relative flex h-10 w-10 shrink-0 items-center justify-center rounded-xl shadow-md',
          iconBg
        )}
      >
        <Icon size={18} className={iconColor} strokeWidth={1.75} />
      </div>

      {/* Text */}
      <div className="relative min-w-0 flex-1">
        <div className="text-sm font-semibold text-white/90">{title}</div>
        <div className="text-xs text-white/40 leading-snug mt-0.5">{description}</div>
      </div>

      {/* Outlined action button */}
      <Button
        onClick={onClick}
        disabled={loading}
        className={cn(
          'relative shrink-0 rounded-lg border px-3 py-1.5 text-xs font-medium transition-all duration-150 h-auto',
          'disabled:pointer-events-none disabled:opacity-40',
          buttonBorder,
          buttonText
        )}
        style={{ boxShadow: loading ? 'none' : buttonGlow }}
      >
        {loading ? (
          <>
            <span className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
            {buttonLabel}
          </>
        ) : (
          buttonLabel
        )}
      </Button>
    </div>
  );
}

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
        imported: number;
        error?: string;
      };
      if (res.success)
        notify(
          t('settings.privacy.importSuccess', {
            count: res.imported,
            plural: res.imported === 1 ? '' : 's',
          }),
          'success'
        );
      else if (res.error) notify(t('settings.privacy.somethingWentWrong'), 'error');
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

  return (
    <div className="space-y-3">
      {/* Export */}
      <ActionCard
        icon={Download}
        iconBg="bg-emerald-600"
        iconColor="text-white"
        glowColor="rgba(16,185,129,0.18)"
        title={t('settings.privacy.exportData')}
        description={t('settings.privacy.exportDataDescription')}
        buttonLabel={t('settings.privacy.export')}
        buttonBorder="border-emerald-500/50"
        buttonText="text-emerald-400"
        buttonGlow="0 0 16px rgba(16,185,129,0.15)"
        loading={!!busy.export}
        onClick={() => void handleExport()}
      />

      {/* Import */}
      <ActionCard
        icon={Upload}
        iconBg="bg-blue-600"
        iconColor="text-white"
        glowColor="rgba(59,130,246,0.18)"
        title={t('settings.privacy.importData')}
        description={t('settings.privacy.importDataDescription')}
        buttonLabel={t('settings.privacy.import')}
        buttonBorder="border-blue-500/50"
        buttonText="text-blue-400"
        buttonGlow="0 0 16px rgba(59,130,246,0.15)"
        loading={!!busy.import}
        onClick={() => void handleImport()}
      />

      {/* Sign Out */}
      <ActionCard
        icon={LogOut}
        iconBg="bg-amber-600"
        iconColor="text-white"
        glowColor="rgba(245,158,11,0.18)"
        title={t('settings.privacy.signOutAll')}
        description={t('settings.privacy.signOutAllDescription')}
        buttonLabel={t('settings.privacy.signOut')}
        buttonBorder="border-amber-500/50"
        buttonText="text-amber-400"
        buttonGlow="0 0 16px rgba(245,158,11,0.15)"
        loading={!!busy.signOut}
        onClick={() => setConfirm({ open: true, action: 'signOut' })}
      />

      {/* Clear History */}
      <ActionCard
        icon={Trash2}
        iconBg="bg-red-600"
        iconColor="text-white"
        glowColor="rgba(239,68,68,0.18)"
        title={t('settings.privacy.clearHistory')}
        description={t('settings.privacy.clearHistoryDescription')}
        buttonLabel={t('settings.privacy.clear')}
        buttonBorder="border-red-500/50"
        buttonText="text-red-400"
        buttonGlow="0 0 16px rgba(239,68,68,0.15)"
        loading={!!busy.clearInteractions}
        onClick={() => setConfirm({ open: true, action: 'clearInteractions' })}
      />

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
