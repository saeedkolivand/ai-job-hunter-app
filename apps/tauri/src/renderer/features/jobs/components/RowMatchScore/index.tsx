import { Gauge } from 'lucide-react';

import type { MatchScore } from '@ajh/shared';
import { Button } from '@ajh/ui';

import { MatchBand } from '@/features/jobs/lib/score';
import { useTranslation } from '@/lib/i18n';
import { useDocuments, useMatchResume } from '@/services';

interface RawDoc {
  _id: string;
  isDefault?: boolean;
}

/** Resolve the default saved résumé's id (`_id`), or the first saved one. */
function useDefaultResumeId(): string | null {
  const { data = [] } = useDocuments();
  const docs = data as unknown as RawDoc[];
  const def = docs.find((d) => d.isDefault) ?? docs[0];
  return def?._id ?? null;
}

/**
 * Per-row match score (#50). Scores this posting against the default résumé
 * ONLY when clicked — no mass auto-scoring, so it never blows up embedding
 * cost across a long list — then shows the Low/Medium/High band (#52).
 */
export function RowMatchScore({ jobId }: { jobId: string }) {
  const { t } = useTranslation();
  const resumeId = useDefaultResumeId();
  const match = useMatchResume();
  const result: (MatchScore & { error?: string }) | undefined = match.data;

  if (result && !result.error) {
    return <MatchBand value={result.combined} />;
  }

  return (
    <Button
      size="sm"
      variant="ghost"
      disabled={!resumeId || match.isPending}
      loading={match.isPending}
      onClick={() => resumeId && match.mutate({ resumeId, jobId })}
      title={resumeId ? t('jobs.scoreHint') : t('jobs.scoreNoResume')}
    >
      {!match.isPending && <Gauge size={11} />}
      {t('jobs.score')}
    </Button>
  );
}
