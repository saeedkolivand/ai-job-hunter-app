import { MarkAppliedButton } from '@/components/job/MarkAppliedButton';
import { normalizeJobUrl } from '@/lib/canonical-job-key';
import { useApplications } from '@/services';

/**
 * "Mark as applied" for the job this run was generated for. The Application is
 * found by normalized job URL (the page holds the RAW ad URL; the stored one is
 * normalized). The bar is hidden while its button renders nothing (no URL, no
 * Application yet, or already applied).
 */
export function LinkedMarkApplied({ jobUrl }: { jobUrl: string | undefined }) {
  const apps = useApplications();
  const wanted = jobUrl ? normalizeJobUrl(jobUrl) : '';
  const application = wanted
    ? apps.data?.find((a) => normalizeJobUrl(a.jobUrl) === wanted)
    : undefined;
  if (!application) return null;
  return (
    <div className="flex shrink-0 items-center justify-end gap-2 border-t border-[var(--border-soft)] px-8 py-3 empty:hidden">
      <MarkAppliedButton applicationId={application.id} status={application.status} />
    </div>
  );
}
