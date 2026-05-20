import { BrowserWindow, ipcMain } from 'electron';
import type { ProgressInfo, UpdateDownloadedEvent, UpdateInfo } from 'electron-updater';

import { createLogger } from '@ajh/core';
import { IPC_CHANNELS } from '@ajh/shared';

const logger = createLogger('updater');

export type UpdateStatus =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'available'; version: string; releaseNotes?: string }
  | { state: 'not-available' }
  | { state: 'downloading'; percent: number }
  | { state: 'downloaded'; version: string }
  | { state: 'error'; message: string };

function broadcast(status: UpdateStatus) {
  BrowserWindow.getAllWindows().forEach((win) => {
    win.webContents.send(IPC_CHANNELS.updater.onStatus, status);
  });
}

export async function setupUpdater() {
  // Lazy import — electron-updater is heavy and should not load at startup
  const { default: pkg } = (await import('electron-updater')) as any;
  const { autoUpdater } = pkg as { autoUpdater: any };

  autoUpdater.autoDownload = false;
  autoUpdater.autoInstallOnAppQuit = true;
  autoUpdater.logger = logger;

  autoUpdater.on('checking-for-update', () => {
    broadcast({ state: 'checking' });
  });

  autoUpdater.on('update-available', (info: UpdateInfo) => {
    broadcast({
      state: 'available',
      version: info.version,
      releaseNotes: typeof info.releaseNotes === 'string' ? info.releaseNotes : undefined,
    });
  });

  autoUpdater.on('update-not-available', () => {
    broadcast({ state: 'not-available' });
  });

  autoUpdater.on('download-progress', (progress: ProgressInfo) => {
    broadcast({ state: 'downloading', percent: Math.round(progress.percent) });
  });

  autoUpdater.on('update-downloaded', (info: UpdateDownloadedEvent) => {
    broadcast({ state: 'downloaded', version: info.version });
  });

  autoUpdater.on('error', (err: Error) => {
    logger.error({ err }, 'auto-updater error');
    broadcast({ state: 'error', message: err.message });
  });

  ipcMain.handle(IPC_CHANNELS.updater.check, async () => {
    await autoUpdater.checkForUpdates();
  });

  ipcMain.handle(IPC_CHANNELS.updater.download, async () => {
    await autoUpdater.downloadUpdate();
  });

  ipcMain.handle(IPC_CHANNELS.updater.install, () => {
    autoUpdater.quitAndInstall(false, true);
  });

  // Silent check 10 s after launch, then every 4 h
  setTimeout(() => void autoUpdater.checkForUpdates().catch(() => {}), 10_000);
  setInterval(() => void autoUpdater.checkForUpdates().catch(() => {}), 4 * 60 * 60 * 1_000);
}
