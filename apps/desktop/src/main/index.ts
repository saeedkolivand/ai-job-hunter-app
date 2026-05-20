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
// Must be set before app.whenReady() — these ensure the cinematic background
// (blur, backdrop-filter, CSS animations, blend modes) renders at full quality
// in the packaged production build, matching the development experience.
//
// electron-builder distributed apps can land on machines where Chromium's GPU
// blocklist disables hardware acceleration. Force-enable it so filter: blur(),
// backdrop-filter, and will-change compositing are always GPU-accelerated.
app.commandLine.appendSwitch('enable-gpu-rasterization');
app.commandLine.appendSwitch('enable-zero-copy');
app.commandLine.appendSwitch('ignore-gpu-blocklist');
// Ensure consistent sRGB colour reproduction across displays.
app.commandLine.appendSwitch('force-color-profile', 'srgb');
// Disable frame rate throttling — keeps CSS animations smooth at all times.
app.commandLine.appendSwitch('disable-frame-rate-limit');

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
