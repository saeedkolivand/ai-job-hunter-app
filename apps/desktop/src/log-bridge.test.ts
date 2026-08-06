/**
 * Unit tests for log-bridge.ts.
 *
 * `@tauri-apps/api/core`'s `invoke` is mocked so we can assert exactly what the
 * bridge sends over IPC without a real Tauri bridge. Each test restores
 * `console.warn`/`console.error` afterward — `installConsoleLogBridge` mutates
 * the global `console` object, so a leaked wrapper would bleed into later tests
 * in this file. `console.info` is exercised by the bridge too (it wraps all
 * three), but the repo's `no-console` lint rule bans app code from calling it
 * at all, so nothing here asserts on it directly — vitest isolates globals per
 * test FILE, so a wrapped-but-unused `console.info` never leaks elsewhere.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { installConsoleLogBridge } from './log-bridge';

describe('installConsoleLogBridge', () => {
  const original = { warn: console.warn, error: console.error };

  beforeEach(() => {
    invoke.mockReset().mockResolvedValue(undefined);
  });

  afterEach(() => {
    console.warn = original.warn;
    console.error = original.error;
  });

  it('still calls the original console.warn — devtools output is unaffected', () => {
    const spy = vi.fn();
    console.warn = spy;
    installConsoleLogBridge();
    console.warn('low disk space', { count: 3 });
    expect(spy).toHaveBeenCalledWith('low disk space', { count: 3 });
  });

  it('forwards console.warn to the plugin, stringifying object args', () => {
    console.warn = vi.fn();
    installConsoleLogBridge();
    console.warn('low disk space', { count: 3 });
    expect(invoke).toHaveBeenCalledWith('plugin:log|log', {
      level: 4,
      message: 'low disk space {"count":3}',
    });
  });

  it('sends NO location, so the Rust record target stays bare `webview`', () => {
    // The whole reason this bridge invokes the command directly instead of
    // using the plugin's `warn()` wrapper: the Rust side builds the target as
    // `webview:{location}` when a location is present, and fern's `level_for`
    // prefix match only walks `::` boundaries — so a single-colon target
    // matches no entry and falls back to the global `Warn`, dropping every
    // forwarded `console.info`. `lib.rs` registers `level_for("webview", Info)`,
    // which only matches when the target is exactly `webview`.
    console.warn = vi.fn();
    installConsoleLogBridge();
    console.warn('anything');
    const payload = invoke.mock.calls[0]?.[1] as Record<string, unknown>;
    expect(payload).not.toHaveProperty('location');
    expect(Object.keys(payload).sort()).toEqual(['level', 'message']);
  });

  it('maps each console level to the plugin LogLevel discriminant', () => {
    console.warn = vi.fn();
    console.error = vi.fn();
    installConsoleLogBridge();
    console.warn('w');
    console.error('e');
    expect(invoke).toHaveBeenNthCalledWith(1, 'plugin:log|log', { level: 4, message: 'w' });
    expect(invoke).toHaveBeenNthCalledWith(2, 'plugin:log|log', { level: 5, message: 'e' });
  });

  it('joins multiple args the way existing call sites use them', () => {
    console.error = vi.fn();
    installConsoleLogBridge();
    console.error('DOCX export failed:', new Error('Save dialog was cancelled'));
    expect(invoke).toHaveBeenCalledWith('plugin:log|log', {
      level: 5,
      message: 'DOCX export failed: Error: Save dialog was cancelled',
    });
  });

  it('falls back to String() for a circular object instead of throwing', () => {
    console.warn = vi.fn();
    installConsoleLogBridge();
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(() => console.warn('circular', circular)).not.toThrow();
    expect(invoke).toHaveBeenCalledWith(
      'plugin:log|log',
      expect.objectContaining({ message: expect.stringContaining('circular') })
    );
  });

  it('swallows a forwarding failure instead of raising an unhandled rejection', async () => {
    invoke.mockRejectedValueOnce(new Error('ipc not ready'));
    console.error = vi.fn();
    installConsoleLogBridge();
    expect(() => console.error('boom')).not.toThrow();
    // Let the rejected forward promise's microtask settle — a missing `.catch`
    // in the bridge would surface as an unhandled rejection here.
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
});
