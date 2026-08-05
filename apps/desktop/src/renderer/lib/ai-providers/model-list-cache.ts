import type { ProviderModelInfo } from '@ajh/shared';

/**
 * Last-known-good cloud model list, per provider + base URL — so a transient
 * fetch failure (or an offline launch) doesn't leave the picker empty. Plain
 * `localStorage` (no persistence dependency): the renderer already reads it
 * directly for the persisted language (see `@ajh/translations`), so this
 * follows the same convention.
 */
const CACHE_PREFIX = 'ajh:model-list-cache:';

function cacheKey(provider: string, baseUrl?: string): string {
  return `${CACHE_PREFIX}${provider}:${baseUrl ?? ''}`;
}

function isModelList(value: unknown): value is ProviderModelInfo[] {
  return (
    Array.isArray(value) && value.every((m) => typeof (m as { name?: unknown })?.name === 'string')
  );
}

/** The provider's last successfully fetched model list, or `undefined` if none is cached. */
export function readModelListCache(
  provider: string,
  baseUrl?: string
): ProviderModelInfo[] | undefined {
  try {
    const raw = localStorage.getItem(cacheKey(provider, baseUrl));
    if (!raw) return undefined;
    const parsed: unknown = JSON.parse(raw);
    return isModelList(parsed) ? parsed : undefined;
  } catch {
    return undefined;
  }
}

/** Persist a freshly-fetched model list as the provider's last-good cache. */
export function writeModelListCache(
  provider: string,
  baseUrl: string | undefined,
  models: ProviderModelInfo[]
): void {
  try {
    localStorage.setItem(cacheKey(provider, baseUrl), JSON.stringify(models));
  } catch {
    // Storage full/unavailable (e.g. private mode) — the cache is a best-effort
    // fallback, not a correctness requirement, so a write failure is silent.
  }
}
