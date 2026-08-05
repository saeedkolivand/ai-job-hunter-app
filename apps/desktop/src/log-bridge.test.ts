/**
 * Unit tests for log-bridge.ts.
 *
 * `@tauri-apps/plugin-log`'s `info`/`warn`/`error` are mocked so we can assert
 * what the bridge forwards without a real Tauri IPC bridge. Each test restores
 * `console.warn`/`console.error` afterward — `installConsoleLogBridge` mutates
 * the global `console` object, so a leaked wrapper would bleed into later tests
 * in this file. `console.info` is exercised by the bridge too (it wraps all
 * three), but the repo's `no-console` lint rule bans app code from calling it
 * at all, so nothing here asserts on it directly — vitest isolates globals per
 * test FILE, so a wrapped-but-unused `console.info` never leaks elsewhere.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const logInfo = vi.fn();
const logWarn = vi.fn();
const logError = vi.fn();

vi.mock('@tauri-apps/plugin-log', () => ({
  info: (...args: unknown[]) => logInfo(...args),
  warn: (...args: unknown[]) => logWarn(...args),
  error: (...args: unknown[]) => logError(...args),
}));

import { installConsoleLogBridge } from './log-bridge';

describe('installConsoleLogBridge', () => {
  const original = { warn: console.warn, error: console.error };

  beforeEach(() => {
    logInfo.mockReset().mockResolvedValue(undefined);
    logWarn.mockReset().mockResolvedValue(undefined);
    logError.mockReset().mockResolvedValue(undefined);
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
    expect(logWarn).toHaveBeenCalledWith('low disk space {"count":3}');
  });

  it('joins multiple args the way existing call sites use them', () => {
    console.error = vi.fn();
    installConsoleLogBridge();
    console.error('DOCX export failed:', new Error('Save dialog was cancelled'));
    expect(logError).toHaveBeenCalledWith('DOCX export failed: Error: Save dialog was cancelled');
  });

  it('falls back to String() for a circular object instead of throwing', () => {
    console.warn = vi.fn();
    installConsoleLogBridge();
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(() => console.warn('circular', circular)).not.toThrow();
    expect(logWarn).toHaveBeenCalledWith(expect.stringContaining('circular'));
  });

  it('swallows a forwarding failure instead of raising an unhandled rejection', async () => {
    logError.mockRejectedValueOnce(new Error('ipc not ready'));
    console.error = vi.fn();
    installConsoleLogBridge();
    expect(() => console.error('boom')).not.toThrow();
    // Let the rejected forward promise's microtask settle — a missing `.catch`
    // in the bridge would surface as an unhandled rejection here.
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
});
