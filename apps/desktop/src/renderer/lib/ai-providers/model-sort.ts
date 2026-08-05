import type { ProviderModelInfo } from '@ajh/shared';

/**
 * Sort newest-first by `createdAt` (epoch ms). An entry missing `createdAt`
 * sorts to the end; if EVERY entry in the list lacks it (e.g. Gemini's
 * catalogue, which reports no creation time at all), every pair compares
 * equal and `Array#sort` is stable (ES2019+) — so the provider's own return
 * order is kept rather than reordered arbitrarily.
 */
export function sortModelsNewestFirst(models: ProviderModelInfo[]): ProviderModelInfo[] {
  return [...models].sort((a, b) => (b.createdAt ?? 0) - (a.createdAt ?? 0));
}
