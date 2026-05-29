import { invoke } from '@tauri-apps/api/core';

export const system = {
  health: () => invoke('system_health'),
  getVersion: () => invoke('system_get_version'),
  getLocale: () => invoke('system_get_locale'),
  setLocale: (locale: string) => invoke('system_set_locale', { locale }),
  getPlatform: () => invoke('system_get_platform'),
  openExternal: (url: string) => invoke('system_open_external', { url }),
  setPerformanceMode: (mode: 'low-memory' | 'balanced' | 'performance') =>
    invoke('system_set_performance_mode', { mode }),
  getMetrics: () => invoke('system_get_metrics'),
  checkBrowser: () => invoke('system_check_browser'),
  openDevtools: () => invoke('system_open_devtools'),
  getProtocolVersion: () => invoke<string>('system_get_protocol_version'),
};
