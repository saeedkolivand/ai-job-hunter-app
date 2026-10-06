import { ExternalLink } from 'lucide-react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, useNotification } from '@ajh/ui';

import { hostOf } from '@/components/job/host-of';
import type { Posting } from '@/features/jobs/types';
import { useMarkNotDuplicate, useOpenExternal } from '@/services';

/**
 * Cross-board cluster "All sources" (ADR-029): every member of this cluster,
 * canonical first. A non-canonical member can be split out via "Not a
 * duplicate" — the tombstone survives every re-scrape.
 */
export function ClusterSources({ posting }: { posting: Posting }) {
  const { t } = useTranslation();
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const split = useMarkNotDuplicate();

  // Canonical first; a member is self when it shares the row's key or url.
  const clusterMembers = posting.clusterMembers ?? [];
  const canonicalKey = posting.clusterId;
  const orderedMembers = [...clusterMembers].sort((a, b) =>
    a.key === canonicalKey ? -1 : b.key === canonicalKey ? 1 : 0
  );

  // Split a wrongly-merged member out of the cluster: tombstone it against every
  // OTHER member so it survives re-scrapes (ADR-029 §h). Success is surfaced only
  // after the mutation resolves.
  const handleSplit = (member: { key: string }) => {
    const otherKeys = clusterMembers.filter((m) => m.key !== member.key).map((m) => m.key);
    if (otherKeys.length === 0) return;
    split.mutate(
      { memberKey: member.key, otherKeys },
      {
        onSuccess: () => notify.success({ message: t('jobs.cluster.splitDone') }),
        onError: () => notify.error({ message: t('jobs.cluster.splitFailed') }),
      }
    );
  };

  // Hooks stay mounted for single-member rows: a split resolves AFTER the refetch
  // drops the cluster to one member, and the mutation observer (which owns the
  // per-call toasts) must outlive that.
  if (clusterMembers.length <= 1) return null;

  return (
    <section
      data-testid={TEST_IDS.jobs.clusterMembers}
      className="mb-4 rounded-lg border border-[var(--border-clear)] p-3"
    >
      <h3 className="mb-2 text-fine-print uppercase tracking-wider text-muted-foreground">
        {t('jobs.cluster.sources')}
      </h3>
      <ul className="space-y-1.5">
        {orderedMembers.map((m) => {
          const canonical = m.key === canonicalKey || m.url === posting.url;
          const boardId = m.board?.trim();
          const label = boardId
            ? t(`jobs.boards.${boardId}`, { defaultValue: boardId })
            : hostOf(m.url);
          return (
            <li key={m.key} className="flex items-center justify-between gap-2">
              <Button
                variant="unstyled"
                onClick={() => openExternal.mutate(m.url)}
                title={t('jobs.cluster.openOn', { source: label })}
                className="flex min-w-0 items-center gap-1.5 text-left text-caption text-foreground/75 hover:text-foreground focus-visible:ring-offset-1"
              >
                <ExternalLink size={11} className="shrink-0 text-foreground/40" />
                <span className="truncate">{label}</span>
              </Button>
              {!canonical && (
                <Button
                  variant="ghost"
                  data-testid={TEST_IDS.jobs.clusterSplitButton}
                  onClick={() => handleSplit(m)}
                  disabled={split.isPending}
                  className="shrink-0 text-[11px]"
                >
                  {t('jobs.cluster.notDuplicate')}
                </Button>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
