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
 * So this wraps `console.info`/`warn`/`error` to ALSO forward the message over
 * the `plugin:log|log` command (which lands in the same rotated file), while
 * still calling the original method so devtools output is unchanged. Every
 * existing `console.warn`/`console.error` call site across the renderer is
 * captured for free — nothing at the call site needs to change.
 */
import { invoke } from '@tauri-apps/api/core';

type Level = 'info' | 'warn' | 'error';

/** `LogLevel` discriminants from the plugin's Rust `commands::log`. */
const PLUGIN_LEVEL: Record<Level, number> = { info: 3, warn: 4, error: 5 };

/**
 * Invoke `plugin:log|log` directly rather than through the plugin's `info()`/
 * `warn()`/`error()` wrappers, and deliberately send NO `location`.
 *
 * The Rust command builds its record target as `webview:{location}` when a
 * location is present and bare `webview` when it isn't. `tauri_plugin_log`
 * filters through `fern`, whose `level_for` prefix match only walks `::`
 * boundaries (`log_impl.rs`'s `find_module`) — so a single-colon
 * `webview:foo@…/log-bridge.ts:12:5` target matches NO `level_for` entry and
 * falls back to the global `Warn`, silently dropping every forwarded
 * `console.info`. A bare `webview` target matches `level_for("webview", Info)`
 * exactly, which is what `src-tauri/src/lib.rs` registers.
 *
 * Nothing is lost by dropping the location: the plugin derives it from the
 * call stack's 4th frame, which — because this bridge is what calls the
 * plugin — always pointed at this file rather than the real `console.*` caller.
 */
function forward(level: Level, message: string): Promise<void> {
  return invoke('plugin:log|log', { level: PLUGIN_LEVEL[level], message });
}

/**
 * Console output that must never reach the log file.
 *
 * Tauri's own JS runtime `console.warn`s once per event dispatched to a callback
 * id it can no longer find. That is one warning PER STREAMED DELTA, and it is
 * emitted from Tauri internals, so there is no call site to fix. In a real
 * support bundle it produced **27,178 of 27,601 lines — 98.5% of the file** from
 * just two stale ids, burying the eight genuinely-failed provider calls in the
 * same session.
 *
 * Dropped HERE rather than in `tauri_plugin_log`'s `filter` because that filter
 * only receives `log::Metadata` (level + target), never the message body — and
 * dropping it renderer-side also saves an IPC round-trip per streamed token,
 * which is the part that actually costs something.
 *
 * Kept deliberately narrow: an exact substring of Tauri's own message, not a
 * pattern that could swallow app warnings. The condition it signals (a listener
 * outliving its registration) is worth knowing about, so the first occurrence
 * per session is still forwarded — see `shouldForward`.
 */
const TAURI_STALE_CALLBACK = "Couldn't find callback id";

/** Whether the stale-callback notice has already been forwarded this session. */
let staleCallbackReported = false;

/** Pure: whether `message` should cross the IPC boundary into the log file.
 *  Exported for tests — this is the whole of the suppression policy. */
export function shouldForward(message: string): boolean {
  if (!message.includes(TAURI_STALE_CALLBACK)) return true;
  if (staleCallbackReported) return false;
  staleCallbackReported = true;
  return true;
}

/** Test-only: reset the once-per-session latch. */
export function resetLogBridgeState(): void {
  staleCallbackReported = false;
}

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
  console[level] = (...args: unknown[]) => {
    original(...args);
    const message = args.map(stringifyArg).join(' ');
    // devtools still shows everything (`original` above ran unconditionally) —
    // only the file/IPC copy is suppressed.
    if (!shouldForward(message)) return;
    // Best-effort: a forwarding failure (e.g. IPC not ready yet) must never
    // break the console itself.
    void forward(level, message).catch(() => {});
  };
}

/** Install once at startup, before any other module logs. Never removed —
 *  lives for the app's lifetime, like `restoreTheme()`/`installDesktopNativeBehaviors()`. */
export function installConsoleLogBridge(): void {
  bridgeLevel('info');
  bridgeLevel('warn');
  bridgeLevel('error');
}
