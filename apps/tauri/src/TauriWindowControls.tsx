import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

export function TauriWindowControls() {
  const [isMaximized, setIsMaximized] = useState(false);
  const win = getCurrentWindow();

  useEffect(() => {
    win.isMaximized().then(setIsMaximized);
    const unlisten = win.onResized(() => {
      win.isMaximized().then(setIsMaximized);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [win]);

  return (
    <div className="app-no-drag flex items-center h-10 shrink-0">
      {/* Minimize */}
      <button
        onClick={() => win.minimize()}
        className="flex h-10 w-11 items-center justify-center text-foreground/50 hover:bg-white/10 hover:text-foreground transition-colors"
        aria-label="Minimize"
      >
        <svg width="11" height="1" viewBox="0 0 11 1" fill="currentColor">
          <rect width="11" height="1.5" y="0.25" />
        </svg>
      </button>

      {/* Maximize / Restore */}
      <button
        onClick={() => (isMaximized ? win.unmaximize() : win.maximize())}
        className="flex h-10 w-11 items-center justify-center text-foreground/50 hover:bg-white/10 hover:text-foreground transition-colors"
        aria-label={isMaximized ? 'Restore' : 'Maximize'}
      >
        {isMaximized ? (
          <svg
            width="10"
            height="10"
            viewBox="0 0 10 10"
            fill="none"
            stroke="currentColor"
            strokeWidth="1"
          >
            <rect x="2.5" y="0.5" width="7" height="7" />
            <polyline points="0.5,2.5 0.5,9.5 7.5,9.5" />
          </svg>
        ) : (
          <svg
            width="10"
            height="10"
            viewBox="0 0 10 10"
            fill="none"
            stroke="currentColor"
            strokeWidth="1"
          >
            <rect x="0.5" y="0.5" width="9" height="9" />
          </svg>
        )}
      </button>

      {/* Close */}
      <button
        onClick={() => win.close()}
        className="flex h-10 w-11 items-center justify-center text-foreground/50 hover:bg-red-500 hover:text-white transition-colors"
        aria-label="Close"
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 10 10"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.2"
        >
          <line x1="1" y1="1" x2="9" y2="9" />
          <line x1="9" y1="1" x2="1" y2="9" />
        </svg>
      </button>
    </div>
  );
}
