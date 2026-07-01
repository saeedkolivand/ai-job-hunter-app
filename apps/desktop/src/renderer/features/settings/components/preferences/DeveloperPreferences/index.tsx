import { Bug, Terminal } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, SectionLabel, Switch } from '@ajh/ui';

import { useOpenDevtools } from '@/services';
import { useDebugMode, usePreferencesStore } from '@/store/preferences-store';

export function DeveloperPreferences() {
  const { t } = useTranslation();
  const debugMode = useDebugMode();
  const setDebugMode = usePreferencesStore((s) => s.setDebugMode);

  const { mutate: openDevtools, isPending } = useOpenDevtools();

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
      </div>
    </GlassCard>
  );
}
