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
  /**
   * When `false`, render pure presentational badges with NO focusable Button and
   * NO click handler. Required inside the split-view listbox row, which is
   * `role="option"` with `tabIndex={-1}` (active-descendant pattern): an option
   * must not contain focusable descendants, and virtualized rows unmounting a
   * focused chip would drop focus. The interactive "open other source" affordance
   * still lives in the detail pane's "All sources" section. Default `true`.
   */
  interactive?: boolean;
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
  interactive = true,
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
        const badge = <SourceBadge source={boardId ?? label} />;

        // Presentational-only inside a listbox option: no Button, no click, so
        // the row stays a single tab stop (see the `interactive` doc above).
        if (!interactive) {
          return (
            <span key={m.key} data-testid={TEST_IDS.jobs.clusterSourceChip} className="inline-flex">
              {badge}
            </span>
          );
        }

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
            {badge}
          </Button>
        );
      })}
    </span>
  );
}
