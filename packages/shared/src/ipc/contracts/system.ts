import type { AppMetrics, Locale, RuntimeHealth } from '../../types/index.js';

export interface SystemContract {
  health(): Promise<RuntimeHealth>;

  getVersion(): Promise<string>;

  getLocale(): Promise<Locale>;

  setLocale(locale: Locale): Promise<void>;

  getPlatform(): Promise<string>;

  /**
   * Best-effort OS accent color. `supported` is true only where we can read it
   * (Windows, macOS); elsewhere `color` is null and the renderer keeps the
   * Default accent. Used by the 'System' accent source in Appearance settings.
   */
  accentColor(): Promise<{ supported: boolean; color: string | null }>;

  openExternal(url: string): Promise<void>;

  setPerformanceMode(mode: 'low-memory' | 'balanced' | 'performance'): Promise<void>;

  /** Whether the app is registered to launch at login (default off). */
  getLaunchAtLogin(): Promise<boolean>;

  /** Enable/disable launch-at-login; resolves to the resulting OS state. */
  setLaunchAtLogin(enabled: boolean): Promise<boolean>;

  /**
   * Push the close-to-tray preference to the shell. When enabled, closing the
   * window hides the app to the tray; when disabled, the window closes / app
   * quits normally. The renderer's preferences store owns the value (no getter).
   */
  setCloseToTray(enabled: boolean): Promise<void>;

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
  accentColor: 'system:accentColor',
  openExternal: 'system:openExternal',
  setPerformanceMode: 'system:setPerformanceMode',
  getLaunchAtLogin: 'system:getLaunchAtLogin',
  setLaunchAtLogin: 'system:setLaunchAtLogin',
  setCloseToTray: 'system:setCloseToTray',
  getMetrics: 'system:getMetrics',
  checkBrowser: 'system:checkBrowser',
  openDevtools: 'system:openDevtools',
  getProtocolVersion: 'system:getProtocolVersion',
} as const;
