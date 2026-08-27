import { Bookmark, ExternalLink, Sparkles, Wand2, X } from 'lucide-react';
import { useNavigate, useRouterState } from '@tanstack/react-router';

import type { AutopilotBestMatch, AutopilotBestMatchSource } from '@ajh/shared';
import { type TFunction, useTranslation } from '@ajh/translations';
import { Button, cn, Tag } from '@ajh/ui';

import { AgencyChip } from '@/components/job/AgencyChip';
import { ClusterSourceChips } from '@/components/job/ClusterSourceChips';
import { ROUTES } from '@/constants/routes';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { formatSalaryRange } from '@/lib/format-salary';
import { MatchBand, matchBandDescriptionKey, scoreTier } from '@/lib/match-band';
import { TrustBadge } from '@/lib/trust-badge';
import { useSessionStore } from '@/store/session-store';

/**
 * Which score scale a row's number is on — mirrors `AutopilotCard`'s own
 * `scoreVariant`/`scoreDetail` treatment (ADR-020 addendum) exactly, just
 * against `AutopilotBestMatch`'s field shapes (`score`/`scoreSource` are
 * REQUIRED here — an unscored cluster never qualifies for this list).
 */
function scoreVariant(match: AutopilotBestMatch): 'coverage' | 'combined' {
  return match.scoreSource === 'combined' ? 'combined' : 'coverage';
}

function scoreDetail(t: TFunction, match: AutopilotBestMatch): string {
  const variant = scoreVariant(match);
  const label = t(`autopilot.scoreLabel.${variant}`);
  const tier = t(matchBandDescriptionKey(scoreTier(match.score, variant).key, variant));
  const provisional = match.scoreProvisional ? ` ${t('autopilot.provisionalScoreHint')}` : '';
  return `${label}: ${tier}${provisional}`;
}

export interface BestMatchRowProps {
  match: AutopilotBestMatch;
  /** Smaller type/padding for the `/autopilot` top-3 preview strip. */
  compact?: boolean;
  /** True only when the currently-RENDERED list mixes both score scales —
   *  see `AutopilotCard`'s `mixedScoreSources`; a label repeated identically
   *  on every row is noise. */
  mixedScoreSources: boolean;
  onView: (match: AutopilotBestMatch) => void;
  onSave: (match: AutopilotBestMatch) => void;
  onApply: (match: AutopilotBestMatch) => void;
  onDismiss: (match: AutopilotBestMatch) => void;
}

/**
 * One cross-autopilot best-match row, shared by the `/best-matches` page and
 * the `/autopilot` preview strip (`components/job/` — feature isolation
 * forbids either feature importing the other's components).
 *
 * A11y: the row's main content sits inside a `<Button>` (the View action),
 * mirroring `AutopilotCard`'s found-job row — so every badge INSIDE it
 * (`TrustBadge`) takes `interactive={false}` (a nested focusable popover
 * trigger would be invalid button-in-button HTML), while `AgencyChip` /
 * `ClusterSourceChips` / the "Found by" source chips sit in a sibling block
 * OUTSIDE that button, same as `AutopilotCard`'s cluster row.
 */
export function BestMatchRow({
  match,
  compact = false,
  mixedScoreSources,
  onView,
  onSave,
  onApply,
  onDismiss,
}: BestMatchRowProps) {
  const { t } = useTranslation();
  const formatRelativeTime = useFormatRelativeTime(t);
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const setAutopilot = useSessionStore((s) => s.setAutopilot);

  const variant = scoreVariant(match);
  const salaryLabel = formatSalaryRange(match.salaryMin, match.salaryMax, match.salaryCurrency);
  const hasClusterOrAgency = match.isAgency || (match.clusterMembers?.length ?? 0) > 1;

  // "Found by" chip → jump to that autopilot's card. Already on `/autopilot`
  // (the preview strip): just set the session focus, no navigation — the
  // page's own effect (AutopilotPage/index.tsx:366-382) picks it up and
  // expands/scrolls/highlights the row. Anywhere else (`/best-matches`):
  // set the same focus, then navigate — same target, same effect.
  const handleSourceClick = (source: AutopilotBestMatchSource) => {
    setAutopilot({ focusedId: source.autopilotId, focusedJobUrl: match.url });
    if (pathname !== ROUTES.AUTOPILOT) void navigate({ to: ROUTES.AUTOPILOT });
  };

  return (
    <div
      className={cn(
        'flex flex-col gap-1.5 rounded-xl border border-[var(--border-clear)] bg-card',
        compact ? 'px-3 py-2' : 'px-3.5 py-3'
      )}
    >
      <div className="flex items-center gap-1.5">
        <Button
          variant="unstyled"
          type="button"
          onClick={() => onView(match)}
          title={t('bestMatches.row.view')}
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
        >
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span
                className={cn(
                  'truncate font-semibold text-foreground/80',
                  compact ? 'text-[11px]' : 'text-xs'
                )}
              >
                {match.title}
              </span>
              <TrustBadge trust={match.trust} className="shrink-0" interactive={false} />
            </div>
            <div className="flex items-center gap-1.5 text-[10px] text-foreground/40">
              <span className="truncate">{match.company}</span>
              {match.location && <span className="shrink-0 truncate">· {match.location}</span>}
              {salaryLabel && <span className="shrink-0">· {salaryLabel}</span>}
              <span className="shrink-0">
                · {t('bestMatches.row.discovered', { time: formatRelativeTime(match.foundAt) })}
              </span>
            </div>
          </div>
          <span className="inline-flex shrink-0 items-center gap-0.5" title={scoreDetail(t, match)}>
            {mixedScoreSources && (
              <span
                aria-hidden="true"
                className="shrink-0 text-[8px] font-semibold uppercase tracking-wider text-foreground/40"
              >
                {t(`autopilot.scoreAbbr.${variant}`)}
              </span>
            )}
            {match.scoreProvisional && (
              <span aria-hidden="true" className="text-[11px] leading-none text-foreground/35">
                ~
              </span>
            )}
            <MatchBand
              value={match.score}
              variant={variant}
              muted={match.scoreProvisional}
              describe={false}
            />
            <span className="sr-only">: {scoreDetail(t, match)}</span>
          </span>
          <ExternalLink size={11} className="shrink-0 text-foreground/25" />
        </Button>
        <Button
          onClick={() => onSave(match)}
          title={t('bestMatches.row.save')}
          aria-label={t('bestMatches.row.save')}
          className="flex shrink-0 items-center gap-1 rounded-lg border-transparent bg-muted px-2 py-1 text-[10px] font-medium text-foreground/60 transition-colors hover:text-foreground/85 h-auto"
        >
          <Bookmark size={10} />
        </Button>
        <Button
          onClick={() => onApply(match)}
          title={t('bestMatches.row.apply')}
          className="flex shrink-0 items-center gap-1 rounded-lg border-transparent bg-brand/10 px-2 py-1 text-[10px] font-medium text-brand-soft transition-colors hover:bg-brand/20 h-auto"
        >
          <Wand2 size={10} /> {t('bestMatches.row.apply')}
        </Button>
        <Button
          onClick={() => onDismiss(match)}
          title={t('bestMatches.row.dismiss')}
          aria-label={t('bestMatches.row.dismiss')}
          className="flex shrink-0 items-center gap-1 rounded-lg border-transparent bg-transparent px-1.5 py-1 text-foreground/30 transition-colors hover:text-foreground/70 h-auto"
        >
          <X size={11} />
        </Button>
      </div>

      {(hasClusterOrAgency || match.sources.length > 0) && (
        <div className="flex flex-wrap items-center gap-1.5 pl-0.5">
          {match.isAgency && <AgencyChip className="px-1 py-0 text-[9px]" />}
          <ClusterSourceChips
            members={match.clusterMembers}
            selfKey={match.key}
            selfUrl={match.url}
          />
          {match.sources.map((source) => {
            const label = t('bestMatches.row.foundBy', { name: source.autopilotName });
            return (
              <Button
                key={source.autopilotId}
                variant="unstyled"
                onClick={() => handleSourceClick(source)}
                title={label}
                aria-label={label}
                className="rounded-full focus-visible:ring-offset-1"
              >
                <Tag
                  color={source.paused ? undefined : 'default'}
                  className="px-1.5 py-0 text-[9px]"
                >
                  {label}
                  {source.paused && t('bestMatches.row.pausedSuffix')}
                </Tag>
              </Button>
            );
          })}
        </div>
      )}

      {/* LLM-generated — plain text only, never markdown/HTML. Same treatment
          as AutopilotCard's AI note (index.tsx:882-901). */}
      {!compact && match.assistantNotes && (
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
              title={match.assistantNotes}
              className="line-clamp-2 text-[10px] leading-relaxed text-foreground/70"
            >
              {match.assistantNotes}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * Optimistic post-Dismiss placeholder — see `useBestMatchActions`'s doc
 * comment for why "Undo" is client-side only (there is no server-side
 * "un-dismiss" IPC call). Same footprint/padding as the row it replaces so
 * the list doesn't jump.
 */
export function DismissedBestMatchRow({
  compact = false,
  onUndo,
}: {
  compact?: boolean;
  onUndo: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className={cn(
        'flex items-center justify-between rounded-xl border border-[var(--border-clear)] bg-card text-[10px] text-foreground/40',
        compact ? 'px-3 py-2' : 'px-3.5 py-3'
      )}
    >
      <span>{t('bestMatches.row.dismissed')}</span>
      <Button
        variant="unstyled"
        onClick={onUndo}
        className="rounded px-1.5 py-0.5 text-brand-soft hover:underline focus-visible:ring-offset-1"
      >
        {t('bestMatches.row.undo')}
      </Button>
    </div>
  );
}
