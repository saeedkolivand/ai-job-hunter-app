import { Sparkles } from 'lucide-react';
import { type ComponentType, useEffect, useState } from 'react';

import { useTranslation } from '@ajh/translations';

import { onWindowControlsRegistered } from '@/lib/window-controls-registry';

import { NotificationBell } from './NotificationBell';

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
      {/* Left spacer — mirrors the window-controls width so the right cluster has
          something to balance against. Mac: 80px traffic lights · Windows/Linux:
          3 × 44px control buttons = 132px */}
      {isMac ? <div className="w-20" /> : <div className="w-[132px]" />}

      {/* Title is an absolutely-centered overlay so it stays centered on the whole
          bar regardless of how wide the left/right clusters are (the bell widens
          the right side). `pointer-events-none` lets the drag region pass through
          — the title isn't interactive, so nothing is lost. */}
      <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
        <div className="flex items-center gap-2 text-xs font-medium text-foreground/70">
          <Sparkles size={14} className="opacity-80" />
          <span className="text-gradient font-semibold tracking-wide">{t('app.title')}</span>
          <span className="text-foreground/40">·</span>
          <span className="text-foreground/40">{t('app.tagline')}</span>
        </div>
      </div>

      {/* Right cluster: notification bell + platform window controls. `app-no-drag`
          keeps both clickable inside the drag region. */}
      <div className="app-no-drag flex items-center gap-1">
        <NotificationBell />
        {WindowControls && !isMac ? <WindowControls /> : <div className="w-20" />}
      </div>
    </div>
  );
}
