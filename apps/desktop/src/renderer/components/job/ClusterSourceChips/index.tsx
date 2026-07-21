import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, cn, SourceBadge } from '@ajh/ui';

import { useOpenExternal } from '@/services';

/** A single cross-board cluster member (opaque key + its board + url). Mirrors
 *  the backend-attached shape on `Posting`/`AutopilotFoundJob` (ADR-029). */
export interface ClusterMember {
  key: string;
  board?: string;
  url: string;
}

interface ClusterSourceChipsProps {
  /** Every member of the cluster (includes self, on the canonical row). */
  members?: ClusterMember[];
  /** The canonical row's own cluster key — used to drop self from the chips. */
  selfKey?: string;
  /** The canonical row's own url — a second self-match signal (Posting rows
   *  carry no member key of their own). */
  selfUrl?: string;
  className?: string;
}

/** Best-effort host label when a member carries no board id. */
function hostOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

/**
 * Cross-board cluster source chips (ADR-029): given a canonical posting/found-job
 * with `clusterMembers`, render an "Also on" label plus one chip per NON-SELF
 * member. A member is self when it shares the row's key or url. Each chip is a
 * keyboard-reachable `@ajh/ui` `Button` (native focus ring) wrapping a
 * `SourceBadge` for per-platform colour/icon; clicking it opens that member's
 * url through the `useOpenExternal` service hook (never `window.open` directly).
 * Renders nothing when the cluster has no other member.
 */
export function ClusterSourceChips({
  members,
  selfKey,
  selfUrl,
  className,
}: ClusterSourceChipsProps) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();

  const others = (members ?? []).filter((m) => m.key !== selfKey && m.url !== selfUrl);
  if (others.length === 0) return null;

  return (
    <span className={cn('inline-flex flex-wrap items-center gap-1.5', className)}>
      <span className="text-fine-print text-foreground/45">{t('jobs.cluster.alsoOn')}</span>
      {others.map((m) => {
        const boardId = m.board?.trim();
        const label = boardId
          ? t(`jobs.boards.${boardId}`, { defaultValue: boardId })
          : hostOf(m.url);
        return (
          <Button
            key={m.key}
            variant="unstyled"
            data-testid={TEST_IDS.jobs.clusterSourceChip}
            onClick={() => openExternal.mutate(m.url)}
            aria-label={t('jobs.cluster.openOn', { source: label })}
            title={t('jobs.cluster.openOn', { source: label })}
            className="rounded-full focus-visible:ring-offset-1"
          >
            <SourceBadge source={boardId ?? label} />
          </Button>
        );
      })}
    </span>
  );
}
