import { useCallback, useState } from 'react';

/**
 * Where the last PDF/DOCX résumé export was saved — feeds `AtsViewPanel`.
 * `track(true, run)` clears the path when a résumé export starts and records the path `run`
 * resolves with; a cover letter (`false`) leaves the path alone, TXT resolves no path, and a
 * throwing export leaves it cleared (panel hidden).
 */
export function useResumeExportPath() {
  const [path, setPath] = useState<string | null>(null);
  const track = useCallback(async (isResume: boolean, run: () => Promise<string | void>) => {
    if (isResume) setPath(null);
    const saved = await run();
    if (isResume && saved) setPath(saved);
  }, []);
  return { path, track };
}

/** The `track` function of {@link useResumeExportPath}. */
export type TrackResumeExport = ReturnType<typeof useResumeExportPath>['track'];
