import { Star } from 'lucide-react';
import { useFormContext, useWatch } from 'react-hook-form';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, SetupHint, Switch, useNotification } from '@ajh/ui';

import type { WizardState } from '@/features/autopilot/types';
import { useSetStarred, useWatchedCompanies } from '@/services/use-discovery';

/**
 * Wizard board-step option (ADR-030 §e): "My watched ATS companies". Toggling it
 * on sets `watchedCompaniesOnly` on the target, so each scheduled run resolves
 * the user's currently-starred companies at run time instead of the curated
 * seed. When on, the current watched set is shown inline (each unstarrable);
 * when empty, a hint points the user to the jobs scrape form to star companies.
 *
 * All company text (`displayName`/`slug`) is rendered ONLY as JSX text nodes —
 * these are scraped, attacker-influenceable strings (Feature-B advisory).
 */
export function WatchedCompaniesField() {
  const { t } = useTranslation();
  const notify = useNotification();
  const { control, setValue } = useFormContext<WizardState>();
  const watchedOnly = useWatch({ control, name: 'watchedCompaniesOnly' });

  const { data: watched } = useWatchedCompanies();
  const setStarred = useSetStarred();
  const companies = watched ?? [];

  const unstar = (atsKind: string, slug: string) => {
    setStarred.mutate(
      { atsKind, slug, starred: false },
      { onError: () => notify.error({ message: t('jobs.discovery.starFailed') }) }
    );
  };

  return (
    <div
      data-testid={TEST_IDS.autopilot.watchedCompaniesToggle}
      className="rounded-xl border border-[var(--border-clear)] bg-card px-4 py-3 space-y-2"
    >
      <Switch
        label={t('autopilot.wizard.target.watched.label')}
        checked={watchedOnly}
        onCheckedChange={(next) => setValue('watchedCompaniesOnly', next, { shouldDirty: true })}
      />
      <p className="text-caption text-foreground/70">
        {t('autopilot.wizard.target.watched.description')}
      </p>

      {watchedOnly &&
        (companies.length === 0 ? (
          <SetupHint message={t('autopilot.wizard.target.watched.emptyHint')} />
        ) : (
          <ul className="space-y-1">
            {companies.map((c) => (
              <li
                key={`${c.atsKind}:${c.slug}`}
                className="flex items-center gap-2 rounded-lg bg-field px-2.5 py-1.5"
              >
                <span className="min-w-0 flex-1 truncate text-xs text-foreground/80">
                  {c.displayName || c.slug}
                </span>
                <span className="shrink-0 rounded bg-muted px-1 py-px text-[9px] uppercase tracking-wide text-foreground/45">
                  {c.atsKind}
                </span>
                <Button
                  variant="ghost"
                  aria-pressed={true}
                  aria-label={t('autopilot.wizard.target.watched.unstar', {
                    company: c.displayName || c.slug,
                  })}
                  onClick={() => unstar(c.atsKind, c.slug)}
                  className="h-auto shrink-0 rounded p-1 text-amber-400 hover:text-amber-300"
                >
                  <Star size={13} aria-hidden="true" fill="currentColor" />
                </Button>
              </li>
            ))}
          </ul>
        ))}
    </div>
  );
}
