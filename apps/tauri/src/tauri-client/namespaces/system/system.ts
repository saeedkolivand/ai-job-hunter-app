import { invoke } from '@tauri-apps/api/core';

export const system = {
  health: () => invoke('system_health'),
  getVersion: () => invoke('system_get_version'),
  getLocale: () => invoke('system_get_locale'),
  setLocale: (locale: string) => invoke('system_set_locale', { locale }),
  getPlatform: () => invoke('system_get_platform'),
  accentColor: () => invoke<{ supported: boolean; color: string | null }>('system_accent_color'),
  openExternal: (url: string) => invoke('system_open_external', { url }),
  setPerformanceMode: (mode: 'low-memory' | 'balanced' | 'performance') =>
    invoke('system_set_performance_mode', { mode }),
  getLaunchAtLogin: () => invoke<boolean>('system_get_launch_at_login'),
  setLaunchAtLogin: (enabled: boolean) =>
    invoke<boolean>('system_set_launch_at_login', { enabled }),
  setCloseToTray: (enabled: boolean) => invoke<void>('system_set_close_to_tray', { enabled }),
  getMetrics: () => invoke('system_get_metrics'),
  checkBrowser: () => invoke('system_check_browser'),
  openDevtools: () => invoke('system_open_devtools'),
  getProtocolVersion: () => invoke<string>('system_get_protocol_version'),
};
