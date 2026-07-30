// Pure assembly-line belt-scrub math. DOM measurement + state lives in
// components/agent-system/hooks.ts (useBeltScrub).

import { STATIONS } from '@/data/agent-fleet';

// The assembly line has as many stops as STATIONS declares — never a
// hardcoded literal, so adding/removing a station can't silently desync the
// "N / STATION_COUNT" labels from the actual data.
export const STATION_COUNT = STATIONS.length;

/** Scroll progress (0-1) through the belt section's scroll runway. */
export function beltProgress(
  sectionTop: number,
  sectionHeight: number,
  viewportHeight: number
): number {
  const runway = sectionHeight - viewportHeight;
  const scrolled = Math.min(runway, Math.max(0, -sectionTop));
  return runway > 0 ? scrolled / runway : 0;
}

/** The diff token's x position, interpolated between the first and last station centers. */
export function activeStationX(firstCenter: number, lastCenter: number, progress: number): number {
  return firstCenter + (lastCenter - firstCenter) * progress;
}

/** Index of the station center closest to activeX (argmin |center - activeX|). */
export function nearestStationIndex(centers: readonly number[], activeX: number): number {
  let step = 0;
  let minDiff = Infinity;
  centers.forEach((c, i) => {
    const diff = Math.abs(c - activeX);
    if (diff < minDiff) {
      minDiff = diff;
      step = i;
    }
  });
  return step;
}
