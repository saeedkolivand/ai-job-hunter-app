import { Info } from 'lucide-react';

import type { BoardCatalogEntry } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';

/** How many curated company names to show inline before collapsing into "+N more". */
const SHOWN_LIMIT = 5;

interface SeededCompaniesNoteProps {
  /** The currently-selected listed boards (as catalog entries). */
  boards: readonly BoardCatalogEntry[];
}

/**
 * Wizard/picker disclosure (#621): a company-scoped ATS board (Greenhouse/Lever/
 * Ashby/…) doesn't take a free-text search — it queries a fixed curated set of
 * companies. This names them so the user isn't surprised by what gets searched.
 * One line per selected board that carries `seededCompanies`, prefixed with the
 * board's label so multiple selected seeded boards each stay unambiguously
 * attributed. The full list is available on hover via the native title tooltip
 * — deliberately not a custom expandable widget (MVP truncation is enough here).
 *
 * Shared by the jobs manual picker (`ScrapeForm`) and the autopilot wizard
 * (`StepTarget`) — the engine's curated-seed fallback fires for both — so it
 * lives outside both feature dirs (no cross-feature import), mirroring
 * `LocationFilterNote`.
 */
export function SeededCompaniesNote({ boards }: SeededCompaniesNoteProps) {
  const { t } = useTranslation();
  const seeded = boards.filter((b) => (b.seededCompanies?.length ?? 0) > 0);
  if (seeded.length === 0) return null;

  return (
    <div role="note" className="mt-2 space-y-1.5 text-[10px] text-foreground/70">
      {seeded.map((board) => {
        const companies = board.seededCompanies ?? [];
        const shown = companies.slice(0, SHOWN_LIMIT);
        const remaining = companies.length - shown.length;
        // The comma before "+N more" lives inside the translated `more` string
        // itself (locale-owned punctuation), so it's built as one plain string
        // rather than adjacent JSX nodes (JSX collapses inter-line whitespace,
        // which would make the separator unreliable).
        const companiesText =
          shown.join(', ') +
          (remaining > 0
            ? t('autopilot.wizard.target.seededCompanies.more', { count: remaining })
            : '') +
          '.';
        return (
          <div key={board.id} className="flex items-start gap-1.5" title={companies.join(', ')}>
            <Info size={11} aria-hidden="true" className="mt-0.5 shrink-0" />
            <p>
              <span className="font-medium text-foreground/80">
                {t(`jobs.boards.${board.id}`, { defaultValue: board.id })}:
              </span>{' '}
              {t('autopilot.wizard.target.seededCompanies.hint')} {companiesText}
            </p>
          </div>
        );
      })}
    </div>
  );
}
