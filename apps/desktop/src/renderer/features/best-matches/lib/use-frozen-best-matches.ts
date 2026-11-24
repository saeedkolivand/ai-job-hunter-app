import { useCallback, useEffect, useState } from 'react';

import type { AutopilotBestMatchesResult } from '@ajh/shared';

import { useBestMatches } from '@/services';

/**
 * The Best Matches list, frozen for the visit (#1449). The backend recomputes the capped list on
 * every call and excludes dismissed jobs, so a background refetch after a dismissal would pull
 * replacements in from the larger pool and re-sort rows under the pointer. This keeps the first
 * result as the displayed batch; only an explicit {@link refresh} ("next batch") replaces it.
 *
 * While the snapshot is still empty it keeps following live data, so a first autopilot run that
 * lands after the page opened is not hidden behind a stale empty list.
 */
export function useFrozenBestMatches() {
  const query = useBestMatches();
  const { data: live, refetch } = query;
  const [snapshot, setSnapshot] = useState<AutopilotBestMatchesResult | null>(null);

  useEffect(() => {
    if (live && (snapshot === null || snapshot.matches.length === 0)) setSnapshot(live);
  }, [live, snapshot]);

  const refresh = useCallback(async () => {
    const result = await refetch();
    if (result?.data) setSnapshot(result.data);
  }, [refetch]);

  return { ...query, data: snapshot ?? live, refresh };
}
