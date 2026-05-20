/**
 * Shared mutable store for startup timing.
 * Written by index.ts, read by the IPC router.
 * Kept in a separate module to avoid circular imports.
 */

let _startupMs: number | null = null;

export function setStartupMs(ms: number): void {
  if (_startupMs === null) _startupMs = ms;
}

export function getStartupMs(): number | null {
  return _startupMs;
}
