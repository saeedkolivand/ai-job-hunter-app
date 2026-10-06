import { Check, ExternalLink, Eye, Sparkles, Wand2 } from 'lucide-react';

import type { AutopilotFoundJob } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, cn, Tag } from '@ajh/ui';

import { AgencyChip } from '@/components/job/AgencyChip';
import { ClusterSourceChips } from '@/components/job/ClusterSourceChips';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { MatchBand } from '@/lib/match-band';
import { TrustBadge } from '@/lib/trust-badge';

import { scoreDetail, scoreVariant } from './found-jobs';

const STATUS_TAG = 'rounded-full px-1.5 py-0.5 text-[8px] uppercase tracking-wider';

interface Props {
  job: AutopilotFoundJob;
  highlighted: boolean;
  /** The list mixes both score scales — each row then names its own metric. */
  mixedScoreSources: boolean;
  viewed: boolean;
  splitPending: boolean;
  onOpen: (job: AutopilotFoundJob) => void;
  onApply: (job: AutopilotFoundJob) => void;
  onSplitCluster: (job: AutopilotFoundJob) => void;
}

/** One found-job row: title/badges/score, the apply action, cluster row and AI note. */
export function FoundJobRow({
  job,
  highlighted,
  mixedScoreSources,
  viewed,
  splitPending,
  onOpen,
  onApply,
  onSplitCluster,
}: Props) {
  const { t } = useTranslation();
  const formatRelativeTime = useFormatRelativeTime(t);

  return (
    <div
      data-job-url={job.url}
      className={cn(
        'flex flex-col gap-1 px-3 py-2 transition-colors hover:bg-muted',
        highlighted && 'ring-2 ring-inset ring-brand/60'
      )}
    >
      <div className="flex items-center gap-2">
        <Button
          variant="unstyled"
          type="button"
          onClick={() => onOpen(job)}
          title={t('autopilot.viewJob')}
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
        >
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span className="truncate text-[11px] text-foreground/80">{job.title}</span>
              {job.isNew && (
                <span className="shrink-0 rounded-full bg-brand/15 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-wider text-brand-soft">
                  {t('autopilot.badge.new')}
                </span>
              )}
              {job.applied && (
                <span className="flex shrink-0 items-center gap-0.5 rounded-full bg-emerald-400/15 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-wider text-emerald-300">
                  <Check size={8} /> {t('autopilot.badge.applied')}
                </span>
              )}
              {viewed && (
                <Tag color="blue" icon={<Eye size={7} />} className={STATUS_TAG}>
                  {t('jobs.viewed')}
                </Tag>
              )}
              {/* interactive=false: this whole row is already a <Button> (handleJobClick) —
                  a nested focusable popover trigger would be invalid HTML (button-in-button). */}
              <TrustBadge trust={job.trust} className={STATUS_TAG} interactive={false} />
              {/* Board coverage is uneven — several boards ship no publish
                  date — so absence renders nothing rather than "NaN ago".
                  Same helper + namespace the Jobs page uses for postedAt
                  (PostingListItem/index.tsx:121-122); the title carries the
                  absolute timestamp, mirroring ApplicationRow:231.
                  `typeof === 'number'`, not `job.postedAt &&` — the classic
                  0-&&-JSX footgun (a stray "0" text node) AND the one presence
                  contract shared with `sortFoundJobsByDate`'s dated/undated
                  banding (which already treats 0 as dated). */}
              {typeof job.postedAt === 'number' && (
                <span
                  className="shrink-0 text-[10px] text-foreground/40"
                  title={new Date(job.postedAt).toLocaleString()}
                >
                  · {formatRelativeTime(job.postedAt)}
                </span>
              )}
            </div>
            <div className="flex items-center gap-1.5 text-[10px] text-foreground/40">
              <span className="truncate">{job.company}</span>
              {job.location && <span className="truncate">· {job.location}</span>}
            </div>
          </div>
          {typeof job.score === 'number' && (
            // One wrapper for both cases so the metric label
            // ("Keyword Coverage %" / "Match %") is always
            // announced. A provisional score (audit root cause 6)
            // is computed over a truncated aggregator snippet, so
            // the detail pane's full-text re-score may differ —
            // it additionally gets a muted band (ALL tiers,
            // `muted`, not `subtle` — a provisional HIGH must read
            // muted too, unlike `subtle`'s High-stays-bright
            // contract), a "~" prefix and the caveat in the copy.
            // The hover `title` plus an always-present sr-only
            // span follows the TrustBadge non-interactive
            // precedent (a `title` alone isn't reliably
            // announced). No focusable HoverPopover — this whole
            // row is already a <Button>; a focusable popover
            // trigger nested in it would be invalid
            // button-in-button HTML.
            <span className="inline-flex shrink-0 items-center gap-0.5" title={scoreDetail(t, job)}>
              {mixedScoreSources && (
                // The scale this number is on, shown only while
                // the list actually mixes the two. aria-hidden because the
                // sr-only span below already announces the full
                // metric name — this is the sighted-user half of
                // the same fact, not a second announcement.
                <span
                  aria-hidden="true"
                  className="shrink-0 text-[8px] font-semibold uppercase tracking-wider text-foreground/40"
                >
                  {t(`autopilot.scoreAbbr.${scoreVariant(job)}`)}
                </span>
              )}
              {job.scoreProvisional && (
                <span aria-hidden="true" className="text-[11px] leading-none text-foreground/35">
                  ~
                </span>
              )}
              {/* describe={false}: this wrapper owns the copy —
                  the band's own `title` would otherwise win on
                  hover over the badge itself and hide the metric
                  label (and the provisional caveat), and its
                  sr-only suffix would double up with the one
                  below. Same caller-owns-richer-copy split as
                  RowMatchScore. */}
              <MatchBand
                value={job.score}
                variant={scoreVariant(job)}
                muted={job.scoreProvisional}
                describe={false}
              />
              <span className="sr-only">: {scoreDetail(t, job)}</span>
            </span>
          )}
          <ExternalLink size={11} className="shrink-0 text-foreground/25" />
        </Button>
        <Button
          onClick={() => onApply(job)}
          title={t('autopilot.applyJob')}
          className="flex shrink-0 items-center gap-1 rounded-lg border-transparent bg-brand/10 px-2 py-1 text-[10px] font-medium text-brand-soft transition-colors hover:bg-brand/20 h-auto"
        >
          <Wand2 size={10} /> {t('autopilot.applyJob')}
        </Button>
      </div>

      {/* Cross-board cluster row (ADR-029) — agency marker, source
          chips for other boards, and a split action. Kept OUTSIDE
          the row's main <Button> above (interactive chips + split
          would be invalid button-in-button HTML otherwise). */}
      {(job.isAgency || (job.clusterMembers?.length ?? 0) > 1) && (
        <div className="flex flex-wrap items-center gap-1.5 pl-0.5">
          {job.isAgency && <AgencyChip className="px-1 py-0 text-[9px]" />}
          <ClusterSourceChips
            members={job.clusterMembers}
            selfKey={job.clusterId}
            selfUrl={job.url}
          />
          {(job.clusterMembers?.length ?? 0) > 1 && (
            <Button
              variant="unstyled"
              data-testid={TEST_IDS.jobs.clusterSplitButton}
              onClick={() => onSplitCluster(job)}
              disabled={splitPending}
              className="rounded px-1.5 py-0.5 text-[10px] text-foreground/50 transition-colors hover:text-foreground/80 focus-visible:ring-offset-1"
            >
              {t('jobs.cluster.notDuplicate')}
            </Button>
          )}
        </div>
      )}

      {/* LLM-generated — always rendered as plain text, never markdown/HTML.
          Visible "AI note" label (not just the aria-label) so sighted users get
          the same "AI-generated, not fact" cue as the icon-only Sparkles gives
          screen readers. Clamped to 2 lines — a verbose note gets a `title`
          tooltip for the full text instead of dominating the compact row. */}
      {job.assistantNotes && (
        <div
          role="note"
          aria-label={t('autopilot.aiNote')}
          className="ml-0.5 flex items-start gap-1.5 rounded-lg border border-brand/15 bg-brand/5 px-2.5 py-1.5"
        >
          <Sparkles size={10} className="mt-0.5 shrink-0 text-brand-soft" />
          <div className="min-w-0 flex-1">
            <span className="block text-fine-print font-semibold uppercase tracking-wide text-brand-soft">
              {t('autopilot.aiNote')}
            </span>
            <p
              title={job.assistantNotes}
              className="line-clamp-2 text-[10px] leading-relaxed text-foreground/70"
            >
              {job.assistantNotes}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
