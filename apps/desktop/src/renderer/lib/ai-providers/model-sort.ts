import type { ProviderModelInfo } from '@ajh/shared';

/**
 * Sort newest-first by `createdAt` (epoch ms). An entry missing `createdAt`
 * sorts to the end, AFTER every entry that has one — including a genuine
 * `createdAt: 0` (Jan 1 1970 is still a real timestamp). Comparing presence
 * before comparing values matters: `createdAt ?? 0` would conflate "absent"
 * with "epoch zero", so an undated entry ahead of a `{ createdAt: 0 }` one
 * would compare equal and (via `Array#sort`'s stability) keep whatever
 * arbitrary order the provider happened to return them in. If EVERY entry in
 * the list lacks `createdAt` (e.g. Gemini's catalogue, which reports no
 * creation time at all), every pair compares equal and the provider's own
 * return order is kept rather than reordered arbitrarily — the stability
 * argument this function relies on.
 */
export function sortModelsNewestFirst(models: ProviderModelInfo[]): ProviderModelInfo[] {
  return [...models].sort((a, b) => {
    const aCreatedAt = a.createdAt;
    const bCreatedAt = b.createdAt;
    if (aCreatedAt !== undefined && bCreatedAt !== undefined) return bCreatedAt - aCreatedAt;
    if (aCreatedAt !== undefined) return -1;
    if (bCreatedAt !== undefined) return 1;
    return 0;
  });
}
