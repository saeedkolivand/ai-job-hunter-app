/**
 * Bridges the renderer's console into the Tauri file logger (`tauri_plugin_log`,
 * configured in `src-tauri/src/lib.rs`) so a failure a user never opens devtools
 * for still lands in the log file a diagnostics bundle captures.
 *
 * `@tauri-apps/plugin-log`'s `attachConsole()` does NOT do this — verified
 * against the plugin's published source (`attachConsole` wraps `attachLogger`,
 * which listens on the `log://log` event the RUST SIDE emits and echoes it into
 * devtools). That is the opposite direction: Rust logs → devtools, not
 * `console.*` → the log file. There is no plugin API that captures native
 * `console.*` calls.
 *
 * So this wraps `console.info`/`warn`/`error` to ALSO forward the message to the
 * plugin's `info`/`warn`/`error` functions (which invoke the `plugin:log|log`
 * command and land in the same rotated file), while still calling the original
 * method so devtools output is unchanged. Every existing `console.warn`/
 * `console.error` call site across the renderer is captured for free — nothing
 * at the call site needs to change.
 */
import { error as logError, info as logInfo, warn as logWarn } from '@tauri-apps/plugin-log';

type Level = 'info' | 'warn' | 'error';

const FORWARDERS: Record<Level, (message: string) => Promise<void>> = {
  info: logInfo,
  warn: logWarn,
  error: logError,
};

function stringifyArg(arg: unknown): string {
  if (arg instanceof Error) return `${arg.name}: ${arg.message}`;
  if (typeof arg === 'string') return arg;
  try {
    return JSON.stringify(arg);
  } catch {
    return String(arg);
  }
}

function bridgeLevel(level: Level): void {
  const original = console[level].bind(console);
  const forward = FORWARDERS[level];
  console[level] = (...args: unknown[]) => {
    original(...args);
    // Best-effort: a forwarding failure (e.g. IPC not ready yet) must never
    // break the console itself.
    void forward(args.map(stringifyArg).join(' ')).catch(() => {});
  };
}

/** Install once at startup, before any other module logs. Never removed —
 *  lives for the app's lifetime, like `restoreTheme()`/`installDesktopNativeBehaviors()`. */
export function installConsoleLogBridge(): void {
  bridgeLevel('info');
  bridgeLevel('warn');
  bridgeLevel('error');
}
