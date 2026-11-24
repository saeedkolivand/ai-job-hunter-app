import { ArrowDownWideNarrow, RefreshCw, Sparkles } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, ConfirmModal, Dropdown, EmptyState, ErrorState, RowSkeleton } from '@ajh/ui';

import {
  BestMatchRow,
  DismissedBestMatchRow,
  UnavailableBestMatchRow,
} from '@/components/job/BestMatchRow';
import { PageShell } from '@/components/layout/PageShell';
import {
  BEST_MATCHES_SORTS,
  type BestMatchesSortBy,
  sortBestMatches,
} from '@/features/best-matches/lib/sort-best-matches';
import { useBestMatchAvailability } from '@/features/best-matches/lib/use-best-match-availability';
import { useFrozenBestMatches } from '@/features/best-matches/lib/use-frozen-best-matches';
import { useBestMatchActions } from '@/hooks/use-best-match-actions';
import { useAutopilots } from '@/services';

function BestMatchesPage() {
  const { t } = useTranslation();
  const [sortBy, setSortBy] = useState<BestMatchesSortBy>('score');
  const { data, isLoading, isError, isFetching, refresh } = useFrozenBestMatches();
  const { data: autopilots = [] } = useAutopilots();
  const { dismissedKeys, handleView, handleSave, handleApply, handleDismiss, undoDismiss } =
    useBestMatchActions();
  const availability = useBestMatchAvailability(handleApply);

  const matches = data?.matches ?? [];
  const sorted = sortBestMatches(matches, sortBy);
  // Counts come from the backend's uncapped `total`, so a reviewed page is never presented as a
  // fully vetted pool (#1449). `reviewed` is what the user dismissed from THIS batch.
  const total = data?.total ?? matches.length;
  const reviewed = matches.filter((m) => dismissedKeys.has(m.key)).length;
  const mixedScoreSources = new Set(sorted.map((m) => m.scoreSource)).size > 1;
  const salariedCount = matches.filter(
    (m) => typeof m.salaryMin === 'number' || typeof m.salaryMax === 'number'
  ).length;
  // Distinguishes the two genuinely different reasons this list can be empty:
  // no autopilot has ever produced a run to score jobs from, vs. autopilots
  // ARE running but nothing has cleared the qualifying tier bar yet. Conflating
  // them as one generic "no results" would tell a first-time user nothing
  // about what to do next.
  const hasEverRun = autopilots.some((ap) => Boolean(ap.lastRunAt));

  const sortActions = matches.length > 0 && (
    // `role="group"` + aria-label, not `<label for>`: a <label> pointing at the
    // Dropdown's own <button> would REPLACE its accessible name (the current
    // sort) instead of adding context to it — same reasoning as
    // ApplicationsPage's sort control.
    <div role="group" aria-label={t('bestMatches.sort.label')} className="w-40">
      <Dropdown
        options={BEST_MATCHES_SORTS.map((s) => ({ value: s, label: t(`bestMatches.sort.${s}`) }))}
        value={sortBy}
        onChange={(value) => setSortBy(value as BestMatchesSortBy)}
        icon={<ArrowDownWideNarrow size={12} />}
      />
    </div>
  );

  return (
    <PageShell
      title={t('bestMatches.title')}
      subtitle={t('bestMatches.subtitle')}
      actions={sortActions}
    >
      {isLoading ? (
        <div className="space-y-3">
          <RowSkeleton />
          <RowSkeleton />
          <RowSkeleton />
        </div>
      ) : isError ? (
        <ErrorState
          title={t('bestMatches.errorTitle')}
          description={t('bestMatches.errorDescription')}
          onRetry={() => void refresh()}
        />
      ) : matches.length === 0 ? (
        <EmptyState
          icon={Sparkles}
          title={t(
            hasEverRun ? 'bestMatches.empty.noneQualified.title' : 'bestMatches.empty.noRuns.title'
          )}
          description={t(
            hasEverRun
              ? 'bestMatches.empty.noneQualified.description'
              : 'bestMatches.empty.noRuns.description'
          )}
        />
      ) : (
        <div className="space-y-3">
          <div className="flex items-center justify-between gap-3">
            <div className="space-y-0.5 text-[11px] text-foreground/40">
              {total > matches.length && (
                <p>{t('bestMatches.truncated', { shown: matches.length, total })}</p>
              )}
              <p>{t('bestMatches.counts', { reviewed, left: Math.max(total - reviewed, 0) })}</p>
            </div>
            <Button
              onClick={() => void refresh()}
              disabled={isFetching}
              className="flex h-auto shrink-0 items-center gap-1 rounded-lg border-transparent bg-muted px-2 py-1 text-[10px] font-medium text-foreground/60 hover:text-foreground/85"
            >
              <RefreshCw size={10} />
              {t(total > matches.length ? 'bestMatches.nextBatch' : 'bestMatches.refresh')}
            </Button>
          </div>
          {sortBy === 'salary' && (
            <p className="text-[11px] text-foreground/40">
              {t('bestMatches.salaryCaption', { count: salariedCount, total: matches.length })}
            </p>
          )}
          <div className="space-y-2">
            {sorted.map((match) =>
              dismissedKeys.has(match.key) ? (
                <DismissedBestMatchRow
                  key={match.key}
                  onUndo={() => undoDismiss(match.key, match.url)}
                />
              ) : availability.isUnavailable(match.key) ? (
                <UnavailableBestMatchRow key={match.key} title={match.title} />
              ) : (
                <BestMatchRow
                  key={match.key}
                  match={availability.withVerdict(match)}
                  mixedScoreSources={mixedScoreSources}
                  onView={handleView}
                  onSave={handleSave}
                  onApply={availability.handlePrepare}
                  onDismiss={handleDismiss}
                  onCheckAvailability={availability.handleCheck}
                  checking={availability.checking.has(match.key)}
                />
              )
            )}
          </div>
          <ConfirmModal
            open={availability.warning !== null}
            onClose={availability.dismissWarning}
            onConfirm={availability.confirmPrepareAnyway}
            title={t('bestMatches.availability.closedTitle')}
            description={t('bestMatches.availability.closedDescription', {
              title: availability.warning?.title ?? '',
            })}
            confirmText={t('bestMatches.availability.prepareAnyway')}
            variant="warning"
          />
        </div>
      )}
    </PageShell>
  );
}

export { BestMatchesPage };
