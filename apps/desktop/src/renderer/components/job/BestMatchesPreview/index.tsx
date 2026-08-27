import { Sparkles } from 'lucide-react';
import { Link } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';

import { BestMatchRow, DismissedBestMatchRow } from '@/components/job/BestMatchRow';
import { ROUTES } from '@/constants/routes';
import { useBestMatchActions } from '@/hooks/use-best-match-actions';
import { useBestMatches } from '@/services';

/** Top-of-strip cap — a taste of the full `/best-matches` list, not a second
 *  copy of it. */
const PREVIEW_COUNT = 3;

/**
 * Top-3 cross-autopilot best-matches strip for the `/autopilot` page.
 * Self-fetching (own `useBestMatches()` call — React Query dedupes this
 * against `/best-matches`'s own query by key, so no double network cost) and
 * renders NOTHING while there are no qualifying rows, so an autopilot user
 * with nothing yet sees no empty strip above their cards.
 */
export function BestMatchesPreview() {
  const { t } = useTranslation();
  const { data } = useBestMatches();
  const { dismissedKeys, handleView, handleSave, handleApply, handleDismiss, undoDismiss } =
    useBestMatchActions();

  const matches = data?.matches ?? [];
  const preview = matches.slice(0, PREVIEW_COUNT);
  if (preview.length === 0) return null;

  // Computed over the RENDERED subset only (the preview strip), not the full
  // list — see BestMatchRow's `mixedScoreSources` doc.
  const mixedScoreSources = new Set(preview.map((m) => m.scoreSource)).size > 1;

  return (
    <div className="mb-4 space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <Sparkles size={13} className="text-brand-soft" />
          <h2 className="text-xs font-semibold text-foreground/70">{t('bestMatches.title')}</h2>
        </div>
        <Link
          to={ROUTES.BEST_MATCHES}
          className="rounded text-[11px] font-medium text-brand-soft hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50"
        >
          {t('bestMatches.viewAll', { count: data?.total ?? matches.length })}
        </Link>
      </div>
      <div className="space-y-2">
        {preview.map((match) =>
          dismissedKeys.has(match.key) ? (
            <DismissedBestMatchRow
              key={match.key}
              compact
              onUndo={() => undoDismiss(match.key, match.url)}
            />
          ) : (
            <BestMatchRow
              key={match.key}
              match={match}
              compact
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
  );
}
