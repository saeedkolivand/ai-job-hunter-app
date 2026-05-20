/**
 * Electron main entry — INTENTIONALLY THIN.
 *
 * Responsibilities:
 *  - app lifecycle
 *  - window management
 *  - menu / tray
 *  - bootstrap the Application Core + Runtimes
 *  - register IPC routes
 *
 * NEVER:
 *  - run AI inference, OCR, scraping, embeddings, or indexing here.
 */
import { app, BrowserWindow } from 'electron';

import { createLogger } from '@ajh/core';

import { bootstrap } from './bootstrap.js';
import { registerIpc } from './ipc/router.js';
import { installMenu } from './menus.js';
import { applySecurityDefaults } from './security.js';
import { setupUpdater } from './updater.js';
import { createMainWindow } from './window.js';

const logger = createLogger('main');

// ── GPU / rendering flags ──────────────────────────────────────────────────
// Must be set before app.whenReady().
//
// enable-gpu-rasterization: accelerates blur/backdrop-filter compositing.
// enable-zero-copy:         reduces texture upload memory copies.
// force-color-profile:      consistent sRGB across displays.
//
// Intentionally omitted:
//   ignore-gpu-blocklist    — respects driver blocklist; avoids crashes on
//                             machines with known-bad GPU drivers.
//   disable-frame-rate-limit — restores OS/browser power-saving throttling;
//                              reduces idle CPU and battery drain.
app.commandLine.appendSwitch('enable-gpu-rasterization');
app.commandLine.appendSwitch('enable-zero-copy');
app.commandLine.appendSwitch('force-color-profile', 'srgb');

applySecurityDefaults();

let mainWindow: BrowserWindow | null = null;

app.whenReady().then(async () => {
  try {
    const core = await bootstrap();
    registerIpc(core);
    mainWindow = await createMainWindow();
    installMenu(mainWindow);
    if (app.isPackaged) void setupUpdater();
    core.onShuttingDown = async () => {
      /* hook for graceful shutdown */
    };
    app.on('activate', async () => {
      if (BrowserWindow.getAllWindows().length === 0) {
        mainWindow = await createMainWindow();
      }
    });
  } catch (err) {
    logger.error({ err }, 'fatal bootstrap error');
    app.quit();
  }
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

app.on('before-quit', async (e) => {
  // Allow runtimes a chance to flush. Bootstrap installs a top-level shutdown hook.
  e.preventDefault();
  try {
    await (global as { __ajh_shutdown?: () => Promise<void> }).__ajh_shutdown?.();
  } catch {
    /* noop */
  }
  app.exit(0);
});
