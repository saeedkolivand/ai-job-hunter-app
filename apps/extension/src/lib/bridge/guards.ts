/**
 * Shared building blocks for the hand-written reply guards in `results-*.ts`.
 * The extension stays zod-free: zod v4's JIT probe trips AMO's `DANGEROUS_EVAL`
 * lint, so every `*.result` payload is checked by hand, then REBUILT from only
 * the known, defined keys (a guard alone would pass unknown keys through, and
 * vitest's `toEqual` treats an explicit `undefined` key as present).
 */

type Rec = Record<string, unknown>;

/** The payload as a plain record, or null when it is not an object. */
export function asRecord(v: unknown): Rec | null {
  return typeof v === 'object' && v !== null ? (v as Rec) : null;
}

/** True when `x` is absent or a string. */
export function optStr(x: unknown): boolean {
  return x === undefined || typeof x === 'string';
}

/**
 * The `ok:false` / `dispatched:false` tail shared by every refusal that may
 * carry a throttle hint: a string `error`, an optional string `detail`, an
 * optional finite `retryAfterMs`.
 */
export function isRefusalTail(o: Rec): boolean {
  if (typeof o.error !== 'string') return false;
  if (o.detail !== undefined && typeof o.detail !== 'string') return false;
  if (o.retryAfterMs !== undefined && !Number.isFinite(o.retryAfterMs)) return false;
  return true;
}

/** Copy only the listed keys of `src` that are defined. */
export function pickDefined<T extends object, K extends keyof T>(
  src: T,
  keys: readonly K[]
): Pick<T, K> {
  const out = {} as Pick<T, K>;
  for (const k of keys) if (src[k] !== undefined) out[k] = src[k];
  return out;
}

/** `{ ...base, detail?, retryAfterMs? }` — a refusal rebuilt from its known keys. */
export function withRefusalExtras<T extends object>(
  base: T,
  src: { detail?: string; retryAfterMs?: number }
): T {
  return { ...base, ...pickDefined(src, ['detail', 'retryAfterMs'] as const) };
}

/**
 * Read the boolean `enabled` flag from an `autotrack.result` / `autofill.result`
 * payload — any malformed/absent shape degrades to `false` (OFF, the safe
 * default), so a bad reply can never make the extension believe an opt-in is on.
 */
export function readEnabledFlag(payload: unknown): boolean {
  return asRecord(payload)?.enabled === true;
}

/** Read a non-empty string field (a nonce / proof) off a handshake payload. */
export function readHexField(payload: unknown, key: string): string | null {
  const v = asRecord(payload)?.[key];
  return typeof v === 'string' && v.length > 0 ? v : null;
}
