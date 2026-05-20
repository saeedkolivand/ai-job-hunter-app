import path from 'node:path';

import { BrowserWindow, clipboard, Menu, shell } from 'electron';

export interface MainWindowOptions {
  /** AJH_LOW_END_MODE=1 — disables vibrancy and transparency to reduce GPU load. */
  lowEndMode?: boolean;
}

export async function createMainWindow(opts: MainWindowOptions = {}): Promise<BrowserWindow> {
  const { lowEndMode = false } = opts;
  // Vibrancy and transparency are expensive on integrated GPUs.
  // Skip them in low-end mode so the compositor can take the fast path.
  const useMacVibrancy = process.platform === 'darwin' && !lowEndMode;

  const win = new BrowserWindow({
    width: 1440,
    height: 920,
    minWidth: 1100,
    minHeight: 720,
    show: false,
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'hidden',
    titleBarOverlay:
      process.platform !== 'darwin'
        ? { color: '#00000000', symbolColor: '#cfcfdc', height: 36 }
        : undefined,
    backgroundColor: '#0a0a14', // matches design system base
    vibrancy: useMacVibrancy ? 'under-window' : undefined,
    visualEffectState: useMacVibrancy ? 'active' : undefined,
    transparent: useMacVibrancy,
    trafficLightPosition: { x: 16, y: 14 },
    webPreferences: {
      preload: path.join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      sandbox: false,
      nodeIntegration: false,
      webSecurity: true,
      spellcheck: true,
    },
  });

  win.once('ready-to-show', () => win.show());

  // Right-click context menu with Cut / Copy / Paste / Select All.
  // Electron doesn't show this by default — without it users have no way to
  // paste via right-click.
  win.webContents.on('context-menu', (_e, params) => {
    const { isEditable, selectionText } = params;
    const hasSelection = selectionText.trim().length > 0;
    const hasClipboard = clipboard.readText().length > 0;

    if (!isEditable && !hasSelection) return; // nothing useful to show

    const menu = Menu.buildFromTemplate([
      ...(isEditable
        ? [
            { label: 'Cut', role: 'cut' as const, enabled: hasSelection },
            { label: 'Copy', role: 'copy' as const, enabled: hasSelection },
            { label: 'Paste', role: 'paste' as const, enabled: hasClipboard },
            { type: 'separator' as const },
            { label: 'Select All', role: 'selectAll' as const },
          ]
        : [{ label: 'Copy', role: 'copy' as const, enabled: hasSelection }]),
    ]);
    menu.popup({ window: win });
  });

  // Disable Ctrl+R / Cmd+R hard reload — it crashes the app in production
  // because the renderer tries to reload from a URL that no longer exists.
  // F5 and Ctrl+Shift+R are also blocked for the same reason.
  win.webContents.on('before-input-event', (_e, input) => {
    if (
      (input.control || input.meta) &&
      (input.key === 'r' || input.key === 'R' || input.key === 'F5')
    ) {
      _e.preventDefault();
    }
  });

  win.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('https://') || url.startsWith('http://')) void shell.openExternal(url);
    return { action: 'deny' };
  });

  win.webContents.session.webRequest.onHeadersReceived((details, callback) => {
    callback({
      responseHeaders: {
        ...details.responseHeaders,
        'Content-Security-Policy': [
          "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' http://127.0.0.1:11434 ws://127.0.0.1:* http://localhost:*",
        ],
      },
    });
  });

  if (process.env.NODE_ENV === 'development' && process.env.ELECTRON_RENDERER_URL) {
    await win.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    await win.loadFile(path.join(__dirname, '../renderer/index.html'));
  }
  return win;
}
