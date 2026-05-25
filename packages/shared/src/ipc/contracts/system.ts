import type { AppMetrics, Locale, RuntimeHealth } from '../../types/index.js';

export interface SystemContract {
  health(): Promise<RuntimeHealth>;

  getVersion(): Promise<string>;

  getLocale(): Promise<Locale>;

  setLocale(locale: Locale): Promise<void>;

  getPlatform(): Promise<string>;

  openExternal(url: string): Promise<void>;

  setPerformanceMode(mode: 'low-memory' | 'balanced' | 'performance'): Promise<void>;

  getMetrics(): Promise<AppMetrics>;

  checkBrowser(): Promise<{ detected: boolean; path?: string }>;
}

export const SYSTEM_CHANNELS = {
  health: 'system:health',
  getVersion: 'system:getVersion',
  getLocale: 'system:getLocale',
  setLocale: 'system:setLocale',
  getPlatform: 'system:getPlatform',
  openExternal: 'system:openExternal',
  setPerformanceMode: 'system:setPerformanceMode',
  getMetrics: 'system:getMetrics',
  checkBrowser: 'system:checkBrowser',
} as const;
