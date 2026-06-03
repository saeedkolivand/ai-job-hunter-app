import type { AppMetrics, Locale, RuntimeHealth } from '../../types/index.js';

export interface SystemContract {
  health(): Promise<RuntimeHealth>;

  getVersion(): Promise<string>;

  getLocale(): Promise<Locale>;

  setLocale(locale: Locale): Promise<void>;

  getPlatform(): Promise<string>;

  openExternal(url: string): Promise<void>;

  setPerformanceMode(mode: 'low-memory' | 'balanced' | 'performance'): Promise<void>;

  /** Whether the app is registered to launch at login (default off). */
  getLaunchAtLogin(): Promise<boolean>;

  /** Enable/disable launch-at-login; resolves to the resulting OS state. */
  setLaunchAtLogin(enabled: boolean): Promise<boolean>;

  getMetrics(): Promise<AppMetrics>;

  checkBrowser(): Promise<{ detected: boolean; path?: string }>;

  openDevtools(): Promise<void>;

  /** Returns the IPC protocol version string from the Tauri shell. */
  getProtocolVersion(): Promise<string>;
}

export const SYSTEM_CHANNELS = {
  health: 'system:health',
  getVersion: 'system:getVersion',
  getLocale: 'system:getLocale',
  setLocale: 'system:setLocale',
  getPlatform: 'system:getPlatform',
  openExternal: 'system:openExternal',
  setPerformanceMode: 'system:setPerformanceMode',
  getLaunchAtLogin: 'system:getLaunchAtLogin',
  setLaunchAtLogin: 'system:setLaunchAtLogin',
  getMetrics: 'system:getMetrics',
  checkBrowser: 'system:checkBrowser',
  openDevtools: 'system:openDevtools',
  getProtocolVersion: 'system:getProtocolVersion',
} as const;
