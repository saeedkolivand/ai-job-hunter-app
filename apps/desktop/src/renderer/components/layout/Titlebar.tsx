import { Search, Sparkles } from 'lucide-react';

import { Button } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useGetPlatform } from '@/services';
import { useAppStore } from '@/store/app-store';

export function Titlebar() {
  const { t } = useTranslation();
  const togglePalette = useAppStore((s) => s.togglePalette);
  const { data: platform } = useGetPlatform();

  return (
    <div
      className="app-drag relative flex h-10 items-center justify-between px-4 select-none"
      style={{ paddingLeft: platform === 'darwin' ? 80 : 16 }}
      data-tauri-drag-region
    >
      <div className="flex items-center gap-2 text-xs font-medium text-foreground/70">
        <Sparkles size={14} className="opacity-80" />
        <span className="text-gradient font-semibold tracking-wide">{t('app.title')}</span>
        <span className="text-foreground/40">·</span>
        <span className="text-foreground/40">{t('app.tagline')}</span>
      </div>
      <Button
        onClick={togglePalette}
        className="app-no-drag glass-dropdown flex items-center gap-3 rounded-lg px-4 py-2 text-sm text-foreground/70 hover:text-foreground transition-colors"
        aria-label="Open command palette"
      >
        <Search size={14} />
        <span>{t('command.placeholder')}</span>
        <kbd className="ml-2 rounded bg-white/5 px-2 py-0.5 text-[10px] text-foreground/60">⌘K</kbd>
      </Button>
      <div className="w-32" /> {/* spacer to balance the layout */}
    </div>
  );
}
