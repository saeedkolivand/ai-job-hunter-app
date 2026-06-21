import type {
  AppMetrics,
  Locale,
  PerformanceBackendConfig,
  RuntimeHealth,
} from '../../types/index.js';

export interface SystemContract {
  /**
   * Signal the Tauri shell that the renderer has painted its first frame. The
   * shell reveals the main window, closes the native splash, and enforces a
   * 700 ms minimum splash display. Idempotent; safe to call multiple times.
   */
  appReady(): Promise<void>;

  /**
   * Push the renderer's resolved color scheme ('light' | 'dark') to the Tauri
   * shell so the native splash can match the app theme on next launch. Must be
   * called on boot AND on every effective-scheme change.
   */
  setThemeMirror(scheme: 'light' | 'dark'): Promise<void>;

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

  setPerformanceMode(config: PerformanceBackendConfig): Promise<void>;

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

  /**
   * Subscribe to OS accent-color changes (Windows personalization). The shell
   * emits `system:accentChanged` from a WinRT `UISettings::ColorValuesChanged`
   * watcher; the renderer re-pulls {@link accentColor} and re-applies the theme
   * when the accent source is 'system'. No-op on platforms without a watcher
   * (macOS/Linux rely on the window-focus refetch fallback). Returns a sync
   * unsubscribe handle. */
  onAccentChanged(handler: () => void): () => void;
}

export const SYSTEM_CHANNELS = {
  appReady: 'system:appReady',
  setThemeMirror: 'system:setThemeMirror',
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
