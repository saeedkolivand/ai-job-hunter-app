import { Sparkles } from 'lucide-react';
import { type ComponentType, useEffect, useState } from 'react';

import { useTranslation } from '@/lib/i18n';
import { onWindowControlsRegistered } from '@/lib/window-controls-registry';

export function Titlebar() {
  const { t } = useTranslation();
  const [WindowControls, setWindowControls] = useState<ComponentType | null>(null);
  const [isMac, setIsMac] = useState(false);

  useEffect(() => {
    onWindowControlsRegistered((c) => setWindowControls(() => c));
    setIsMac(navigator.userAgent.includes('Mac'));
  }, []);

  return (
    <div
      className="app-drag relative z-[300] flex h-10 select-none items-center justify-between"
      data-tauri-drag-region
    >
      {/* Left spacer — mirrors the right side so the title stays centered */}
      {isMac ? <div className="w-20" /> : <div className="w-4" />}

      <div className="flex items-center gap-2 text-xs font-medium text-foreground/70">
        <Sparkles size={14} className="opacity-80" />
        <span className="text-gradient font-semibold tracking-wide">{t('app.title')}</span>
        <span className="text-foreground/40">·</span>
        <span className="text-foreground/40">{t('app.tagline')}</span>
      </div>

      {WindowControls && !isMac ? <WindowControls /> : <div className="w-20" />}
    </div>
  );
}
