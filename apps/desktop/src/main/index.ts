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
import { rollbackFlags } from './rollback-flags.js';
import { applySecurityDefaults } from './security.js';
import { setStartupMs } from './startup-metrics.js';
import { setupUpdater } from './updater.js';
import { createMainWindow } from './window.js';

const logger = createLogger('main');

let _appReadyAt = 0;

// ── GPU / rendering flags ──────────────────────────────────────────────────
// Must be set before app.whenReady().
//
// AJH_LOW_END_MODE=1 skips the GPU acceleration switches so the OS/driver
// blocklist takes effect naturally — this is the Phase 1 rollback switch.
//
// enable-gpu-rasterization: accelerates blur/backdrop-filter compositing.
// enable-zero-copy:         reduces texture upload memory copies.
// force-color-profile:      consistent sRGB across displays.
//
// Intentionally omitted regardless of mode:
//   ignore-gpu-blocklist    — respects driver blocklist; avoids crashes on
//                             machines with known-bad GPU drivers.
//   disable-frame-rate-limit — restores OS/browser power-saving throttling;
//                              reduces idle CPU and battery drain.
if (!rollbackFlags.lowEndMode) {
  app.commandLine.appendSwitch('enable-gpu-rasterization');
  app.commandLine.appendSwitch('enable-zero-copy');
}
app.commandLine.appendSwitch('force-color-profile', 'srgb');

applySecurityDefaults();

let mainWindow: BrowserWindow | null = null;

app.whenReady().then(async () => {
  _appReadyAt = performance.now();
  try {
    const core = await bootstrap();
    registerIpc(core);
    mainWindow = await createMainWindow({ lowEndMode: rollbackFlags.lowEndMode });

    // Record time from app-ready to window visible.
    const startupMs = performance.now() - _appReadyAt;
    setStartupMs(startupMs);
    logger.info({ startupMs: Math.round(startupMs) }, 'first window ready');

    installMenu(mainWindow);
    if (app.isPackaged) void setupUpdater();
    core.onShuttingDown = async () => {
      /* hook for graceful shutdown */
    };

    // Log process metrics every 60 s so they appear in crash logs / diagnostics.
    setInterval(() => {
      const metrics = app.getAppMetrics();
      const summary = metrics.map((m) => ({
        pid: m.pid,
        type: m.type,
        cpu: m.cpu.percentCPUUsage.toFixed(1),
        memMB: Math.round(m.memory.workingSetSize / 1024),
      }));
      logger.info({ processes: summary }, 'process metrics');
    }, 60_000).unref();

    app.on('activate', async () => {
      if (BrowserWindow.getAllWindows().length === 0) {
        mainWindow = await createMainWindow({ lowEndMode: rollbackFlags.lowEndMode });
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
