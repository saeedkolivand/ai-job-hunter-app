import { Search } from 'lucide-react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

interface ScrapeFooterProps {
  scraping: boolean;
  scrapeOutcome: { ok: boolean; note?: string } | null;
  queryEmpty: boolean;
  /** Selected `auth === 'required'` boards that are not connected yet. */
  unconnectedRequired: string[];
  onStart: () => void;
  onCancel: () => void;
}

/** Footer — pinned; Start is always reachable without scrolling. */
export function ScrapeFooter({
  scraping,
  scrapeOutcome,
  queryEmpty,
  unconnectedRequired,
  onStart,
  onCancel,
}: ScrapeFooterProps) {
  const { t } = useTranslation();
  const blockedByRequiredLogin = unconnectedRequired.length > 0;

  return (
    <div className="shrink-0 border-t border-[var(--border-clear)] px-5 py-3">
      {!scraping && blockedByRequiredLogin && (
        <p
          id="scrape-blocked-hint"
          aria-live="polite"
          className="mb-2 text-[11px] text-amber-400/70"
        >
          {t('jobs.needsLogin.blockedHint', {
            boards: unconnectedRequired
              .map((id) => t(`jobs.boards.${id}`, { defaultValue: id }))
              .join(', '),
          })}
        </p>
      )}
      <div className="flex items-center justify-end gap-2">
        {scraping ? (
          <Button variant="ghost" onClick={onCancel}>
            {t('jobs.cancel')}
          </Button>
        ) : (
          scrapeOutcome && (
            <span
              className={cn(
                'min-w-0 flex-1 truncate text-[11px]',
                scrapeOutcome.ok && !scrapeOutcome.note
                  ? 'text-emerald-400/70'
                  : 'text-amber-400/70'
              )}
            >
              {scrapeOutcome.ok
                ? (scrapeOutcome.note ?? t('jobs.done'))
                : (scrapeOutcome.note ?? t('jobs.failed'))}
            </span>
          )
        )}
        <Button
          variant="primary"
          onClick={onStart}
          disabled={scraping || queryEmpty || blockedByRequiredLogin}
          loading={scraping}
          aria-describedby={!scraping && blockedByRequiredLogin ? 'scrape-blocked-hint' : undefined}
          data-testid={TEST_IDS.jobs.scrapeStartButton}
          className="shrink-0 transition-all duration-150 ease-out"
        >
          {!scraping && <Search size={12} />}
          {scraping ? t('jobs.scraping') : t('jobs.startScrape')}
        </Button>
      </div>
    </div>
  );
}
