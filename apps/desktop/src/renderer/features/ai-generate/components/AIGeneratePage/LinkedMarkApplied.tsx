import { MarkAppliedButton } from '@/components/job/MarkAppliedButton';
import { useApplications } from '@/services';
import { useAiGenerations } from '@/services/use-ai-generations';

/**
 * "Mark as applied" for the job this run was generated for — the saved generation
 * (matched by job URL) carries the linked application id. Renders nothing while no
 * linked, still-`saved` application exists (URL-less ad, not yet saved, applied).
 */
export function LinkedMarkApplied({ jobUrl }: { jobUrl: string | undefined }) {
  const generations = useAiGenerations();
  const applicationId = jobUrl
    ? generations.data?.find((g) => g.jobUrl === jobUrl && g.applicationId)?.applicationId
    : undefined;
  const apps = useApplications();
  if (!applicationId) return null;
  return (
    <div className="flex shrink-0 items-center justify-end gap-2 border-t border-[var(--border-soft)] px-8 py-3">
      <MarkAppliedButton
        applicationId={applicationId}
        status={apps.data?.find((a) => a.id === applicationId)?.status}
      />
    </div>
  );
}
