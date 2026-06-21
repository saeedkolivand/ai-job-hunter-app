import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { EVENT_CHANNELS, type PerformanceBackendConfig } from '@ajh/shared';

import { asyncUnsub } from '../../utils.js';

export const system = {
  appReady: () => invoke<void>('app_ready'),
  setThemeMirror: (scheme: 'light' | 'dark') => invoke<void>('set_theme_mirror', { scheme }),
  health: () => invoke('system_health'),
  getVersion: () => invoke('system_get_version'),
  getLocale: () => invoke('system_get_locale'),
  setLocale: (locale: string) => invoke('system_set_locale', { locale }),
  getPlatform: () => invoke('system_get_platform'),
  accentColor: () => invoke<{ supported: boolean; color: string | null }>('system_accent_color'),
  openExternal: (url: string) => invoke('system_open_external', { url }),
  setPerformanceMode: (config: PerformanceBackendConfig) =>
    invoke('system_set_performance_mode', { config }),
  getLaunchAtLogin: () => invoke<boolean>('system_get_launch_at_login'),
  setLaunchAtLogin: (enabled: boolean) =>
    invoke<boolean>('system_set_launch_at_login', { enabled }),
  setCloseToTray: (enabled: boolean) => invoke<void>('system_set_close_to_tray', { enabled }),
  getMetrics: () => invoke('system_get_metrics'),
  checkBrowser: () => invoke('system_check_browser'),
  openDevtools: () => invoke('system_open_devtools'),
  getProtocolVersion: () => invoke<string>('system_get_protocol_version'),
  // OS accent-color change (Windows personalization). Emitted by the WinRT
  // `UISettings::ColorValuesChanged` watcher — see `platform::accent_watcher`.
  // Payload is unused; the renderer re-pulls `accentColor` on the signal.
  onAccentChanged: (handler: () => void) =>
    asyncUnsub(() => listen(EVENT_CHANNELS.system.accentChanged, () => handler())),
};
