import { ChevronLeft, PanelLeft, Sparkles } from 'lucide-react';
import { type ComponentType, useEffect, useState } from 'react';
import { useNavigate, useRouterState } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { parentRoute } from '@/lib/parent-route';
import { onWindowControlsRegistered } from '@/lib/window-controls-registry';
import { useWindowControls } from '@/services';
import {
  useOnboardingCompleted,
  useSidebarCollapsed,
  useToggleSidebar,
} from '@/store/preferences-store';

import { NotificationBell } from './NotificationBell';

export function Titlebar() {
  const { t } = useTranslation();
  const onboardingCompleted = useOnboardingCompleted();
  const isCollapsed = useSidebarCollapsed();
  const toggleSidebar = useToggleSidebar();
  const [WindowControls, setWindowControls] = useState<ComponentType | null>(null);
  const { toggleMaximize, isMacos } = useWindowControls();

  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const navigate = useNavigate();
  const parent = parentRoute(pathname);

  useEffect(() => {
    onWindowControlsRegistered((c) => setWindowControls(() => c));
  }, []);

  // Own the double-click uniformly on all platforms and suppress Tauri's built-in
  // drag-region handler so we don't double-toggle (Win/Linux fire on mousedown,
  // macOS fires on mouseup — both paths are neutralised by stopPropagation before
  // the document-level listener runs, which is safe under React 19's root-container
  // event delegation where synthetic stopPropagation calls nativeEvent.stopPropagation).
  const handleTitlebarDoubleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0 || e.detail !== 2) return;
    if ((e.target as HTMLElement).closest('.app-no-drag')) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.type === 'mousedown') {
      void toggleMaximize();
    }
  };

  return (
    <div
      className="app-drag relative z-[300] flex h-10 select-none items-center justify-between"
      data-tauri-drag-region
      onMouseDown={handleTitlebarDoubleClick}
      onMouseUp={handleTitlebarDoubleClick}
    >
      {/* Left cluster: fixed-width spacer (mirrors window-controls on the right) holding
          the global back button (detail routes) + the expand-sidebar toggle (shown when
          the sidebar is collapsed). `app-no-drag` keeps them clickable. */}
      <div className={`app-no-drag flex items-center gap-1 pl-1 ${isMacos ? 'w-20' : 'w-[132px]'}`}>
        {parent !== null && (
          <Button
            variant="ghost"
            size="sm"
            aria-label={t('nav.back')}
            onClick={() => void navigate({ to: parent })}
            className="text-foreground/60 hover:text-foreground"
          >
            <ChevronLeft size={16} />
          </Button>
        )}
        {isCollapsed && (
          <Button
            variant="ghost"
            size="sm"
            aria-label={t('nav.expandSidebar')}
            onClick={toggleSidebar}
            className="text-foreground/60 hover:text-foreground"
          >
            <PanelLeft size={16} />
          </Button>
        )}
      </div>

      {/* Title is an absolutely-centered overlay so it stays centered on the whole
          bar regardless of how wide the left/right clusters are. `pointer-events-none`
          lets the drag region pass through — the title isn't interactive. */}
      {onboardingCompleted && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <div className="flex items-center gap-2 text-xs font-medium text-foreground/70">
            <Sparkles size={14} className="opacity-80" />
            <span className="text-gradient font-semibold tracking-wide">{t('app.title')}</span>
            <span className="text-foreground/40">·</span>
            <span className="text-foreground/40">{t('app.tagline')}</span>
          </div>
        </div>
      )}

      {/* Right cluster: notification bell + platform window controls. `app-no-drag`
          keeps both clickable inside the drag region. */}
      <div className="app-no-drag flex items-center gap-1">
        {onboardingCompleted && <NotificationBell />}
        {WindowControls && !isMacos ? <WindowControls /> : <div className="w-20" />}
      </div>
    </div>
  );
}
