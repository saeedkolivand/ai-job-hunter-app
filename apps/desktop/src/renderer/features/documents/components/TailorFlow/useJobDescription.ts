import { useState } from 'react';

import type { AutopilotFoundJob } from '@ajh/shared';

import { useResolveJobUrl } from '@/services';

// A short carried description (e.g. an Adzuna API snippet, ~200–400 chars) is
// worth re-resolving: the URL fetch (which now follows the aggregator redirect)
// may reach the fuller ad. Re-resolve when the carried text is short OR empty,
// then prefer whichever description is longer.
// ponytail: 800-char floor separates aggregator snippets from full ads; raise if
// real full ads legitimately come in shorter.
const SHORT_DESC_FLOOR = 800;

/** The job-ad text the flow works from: carried snippet, upgraded by a URL fetch, overridden by user edits. */
export function useJobDescription(
  job: AutopilotFoundJob,
  onJobDescChange: ((text: string) => void) | undefined
) {
  const initialDesc = (job.description ?? '').trim();
  const resolved = useResolveJobUrl(job.url, initialDesc.length < SHORT_DESC_FLOOR);
  const fetchedDesc = (resolved.data?.description ?? '').trim();
  const [jobDescOverride, setJobDescOverride] = useState<string | null>(null);
  // Combine the local override with the optional host persist callback so the
  // host can react to edits (e.g. debounce-persist to application.jobDescription)
  // without TailorFlow caring about storage details.
  const handleJobDescEdit = (v: string) => {
    setJobDescOverride(v);
    onJobDescChange?.(v);
  };
  const jobDesc =
    jobDescOverride ?? (fetchedDesc.length > initialDesc.length ? fetchedDesc : initialDesc);
  return {
    jobDesc,
    hasDesc: jobDesc.length > 0,
    // Show the loading state only when there's nothing to display yet (no snippet);
    // with a snippet present it renders immediately and upgrades silently on fetch.
    fetchingDesc: !initialDesc && resolved.isLoading,
    handleJobDescEdit,
  };
}
