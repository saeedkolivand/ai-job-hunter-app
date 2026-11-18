import { useCallback, useRef, useState } from 'react';

/**
 * Where the last PDF/DOCX résumé export was saved — feeds `AtsViewPanel`.
 * `track(true, run)` clears the path when a résumé export starts and records the path `run`
 * resolves with; a cover letter (`false`) leaves the path alone, TXT resolves no path, and a
 * throwing export leaves it cleared (panel hidden). A résumé export that was superseded by a
 * newer one never stores its path or clears the newer one's (stale PDF/DOCX after a TXT export).
 */
export function useResumeExportPath() {
  const [path, setPath] = useState<string | null>(null);
  const latest = useRef(0);
  const track = useCallback(async (isResume: boolean, run: () => Promise<string | void>) => {
    const ticket = isResume ? ++latest.current : 0;
    if (isResume) setPath(null);
    try {
      const saved = await run();
      if (isResume && saved && ticket === latest.current) setPath(saved);
    } catch (err) {
      if (isResume && ticket === latest.current) setPath(null);
      throw err;
    }
  }, []);
  return { path, track };
}

/** The `track` function of {@link useResumeExportPath}. */
export type TrackResumeExport = ReturnType<typeof useResumeExportPath>['track'];
