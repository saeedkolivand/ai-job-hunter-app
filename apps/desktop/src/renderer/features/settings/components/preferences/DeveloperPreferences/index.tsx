import { Bug, Terminal } from 'lucide-react';
import { save } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';

import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, SectionLabel, Switch, useNotification } from '@ajh/ui';

import { useExportDiagnostics, useOpenDevtools } from '@/services';
import { useDebugMode, usePreferencesStore } from '@/store/preferences-store';

export function DeveloperPreferences() {
  const { t } = useTranslation();
  const debugMode = useDebugMode();
  const setDebugMode = usePreferencesStore((s) => s.setDebugMode);

  const { mutate: openDevtools, isPending } = useOpenDevtools();
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
        notify.success({ message: t('settings.developer.exportDiagnosticsSaved') });
        revealItemInDir(dest).catch(() => {});
      } else {
        notify.error({ message: t('settings.developer.exportDiagnosticsError') });
      }
    } catch (err) {
      console.error('diagnostics export failed:', err instanceof Error ? err.name : 'unknown');
      notify.error({ message: t('settings.developer.exportDiagnosticsError') });
    }
  };

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <Bug size={15} className="text-foreground/50" />
        <SectionLabel>{t('settings.developer.title')}</SectionLabel>
      </div>

      <div className="space-y-3">
        {/* Debug mode toggle */}
        <Switch
          label={t('settings.developer.debugMode')}
          description={t('settings.developer.debugModeHint')}
          checked={debugMode}
          onCheckedChange={setDebugMode}
        />

        {/* Open DevTools */}
        <Button
          onClick={() => openDevtools()}
          disabled={isPending}
          className="w-full flex items-center gap-2 justify-start rounded-lg border border-foreground/10 bg-transparent px-3 py-2.5 text-[11px] text-foreground/55 hover:border-foreground/10 hover:text-foreground/80 transition-colors h-auto"
        >
          <Terminal size={13} />
          {t('settings.developer.openDevtools')}
        </Button>

        {/* Export diagnostics */}
        <div className="space-y-2 pt-1">
          <p className="text-xs text-foreground/40">
            {t('settings.developer.exportDiagnosticsDesc')}
          </p>
          <Button
            variant="glass"
            className="w-full justify-start gap-2 text-xs"
            loading={exportDiagnostics.isPending}
            onClick={handleExportDiagnostics}
          >
            <Bug size={13} />
            {t('settings.developer.exportDiagnostics')}
          </Button>
        </div>
      </div>
    </GlassCard>
  );
}
