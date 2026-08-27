import { ArrowDownWideNarrow, Sparkles } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Dropdown, EmptyState, ErrorState, RowSkeleton } from '@ajh/ui';

import { BestMatchRow, DismissedBestMatchRow } from '@/components/job/BestMatchRow';
import { PageShell } from '@/components/layout/PageShell';
import {
  BEST_MATCHES_SORTS,
  type BestMatchesSortBy,
  sortBestMatches,
} from '@/features/best-matches/lib/sort-best-matches';
import { useBestMatchActions } from '@/hooks/use-best-match-actions';
import { useAutopilots, useBestMatches } from '@/services';

function BestMatchesPage() {
  const { t } = useTranslation();
  const [sortBy, setSortBy] = useState<BestMatchesSortBy>('score');
  const { data, isLoading, isError, refetch } = useBestMatches();
  const { data: autopilots = [] } = useAutopilots();
  const { dismissedKeys, handleView, handleSave, handleApply, handleDismiss, undoDismiss } =
    useBestMatchActions();

  const matches = data?.matches ?? [];
  const sorted = sortBestMatches(matches, sortBy);
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
          onRetry={() => void refetch()}
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
          {data && data.total > data.matches.length && (
            <p className="text-[11px] text-foreground/40">
              {t('bestMatches.truncated', { shown: data.matches.length, total: data.total })}
            </p>
          )}
          {sortBy === 'salary' && (
            <p className="text-[11px] text-foreground/40">
              {t('bestMatches.salaryCaption', { count: salariedCount, total: matches.length })}
            </p>
          )}
          <div className="space-y-2">
            {sorted.map((match) =>
              dismissedKeys.has(match.key) ? (
                <DismissedBestMatchRow key={match.key} onUndo={() => undoDismiss(match.key)} />
              ) : (
                <BestMatchRow
                  key={match.key}
                  match={match}
                  mixedScoreSources={mixedScoreSources}
                  onView={handleView}
                  onSave={handleSave}
                  onApply={handleApply}
                  onDismiss={handleDismiss}
                />
              )
            )}
          </div>
        </div>
      )}
    </PageShell>
  );
}

export { BestMatchesPage };
