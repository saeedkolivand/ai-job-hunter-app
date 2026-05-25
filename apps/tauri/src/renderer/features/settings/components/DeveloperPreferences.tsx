import { Bug, Terminal } from 'lucide-react';
import { useMutation } from '@tanstack/react-query';

import { Button, GlassCard, SectionLabel } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { useAppClient } from '@/providers/AppClientProvider';
import { useDebugMode, usePreferencesStore } from '@/store/preferences-store';

export function DeveloperPreferences() {
  const { t } = useTranslation();
  const api = useAppClient();
  const debugMode = useDebugMode();
  const setDebugMode = usePreferencesStore((s) => s.setDebugMode);

  const { mutate: openDevtools, isPending } = useMutation({
    mutationFn: () => api.system.openDevtools() as Promise<void>,
  });

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <Bug size={15} className="text-foreground/50" />
        <SectionLabel>{t('settings.developer.title')}</SectionLabel>
      </div>

      <div className="space-y-3">
        {/* Debug mode toggle */}
        <button
          type="button"
          onClick={() => setDebugMode(!debugMode)}
          className={cn(
            'w-full flex items-center justify-between rounded-lg border px-3 py-2.5 transition-all text-left',
            debugMode
              ? 'border-brand/35 bg-brand/8'
              : 'border-white/[0.05] bg-transparent hover:border-white/[0.08]'
          )}
        >
          <div>
            <div
              className={cn(
                'text-[11px] font-medium',
                debugMode ? 'text-foreground/90' : 'text-foreground/55'
              )}
            >
              {t('settings.developer.debugMode')}
            </div>
            <div className="text-[10px] text-foreground/35 mt-0.5">
              {t('settings.developer.debugModeHint')}
            </div>
          </div>
          <div
            className={cn(
              'h-4 w-7 rounded-full transition-colors shrink-0 ml-3 relative',
              debugMode ? 'bg-brand' : 'bg-white/10'
            )}
          >
            <div
              className={cn(
                'absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-transform',
                debugMode ? 'translate-x-3.5' : 'translate-x-0.5'
              )}
            />
          </div>
        </button>

        {/* Open DevTools — only in dev builds; uses private WebKit APIs on macOS */}
        {import.meta.env.DEV && (
          <Button
            onClick={() => openDevtools()}
            disabled={isPending}
            className="w-full flex items-center gap-2 justify-start rounded-lg border border-white/[0.05] bg-transparent px-3 py-2.5 text-[11px] text-foreground/55 hover:border-white/[0.08] hover:text-foreground/80 transition-colors h-auto"
          >
            <Terminal size={13} />
            {t('settings.developer.openDevtools')}
          </Button>
        )}
      </div>
    </GlassCard>
  );
}
