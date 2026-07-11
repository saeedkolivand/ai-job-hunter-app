import { Info } from 'lucide-react';

import type { BoardCatalogEntry } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';

interface LocationFilterNoteProps {
  /** The currently-selected listed boards (as catalog entries). */
  boards: readonly BoardCatalogEntry[];
  /** True once the user has entered a location — the hint is meaningless otherwise. */
  hasLocation: boolean;
}

/**
 * Honest per-board picker hint (PR F, job-search trust program). When a location
 * is entered, it names the SELECTED boards that do NOT narrow results by location
 * on their side (`supportsLocation` falsy). The scrape engine post-filters those
 * boards' results to the requested location on-device (dropping only clear
 * mismatches), so this tells the user upfront which boards "ignore" the location
 * upstream — informational and muted, never a warning.
 *
 * Shared by the jobs manual picker and the autopilot wizard, so it lives outside
 * both feature dirs (no cross-feature import), mirroring `BoardSummaryChips`.
 */
export function LocationFilterNote({ boards, hasLocation }: LocationFilterNoteProps) {
  const { t } = useTranslation();
  if (!hasLocation) return null;

  // Optional flag: an absent / older payload reads as "does not support location"
  // — matching the contract semantics and the engine's conservative post-filter.
  const affected = boards.filter((b) => !b.supportsLocation);
  if (affected.length === 0) return null;

  return (
    <div
      role="note"
      className="mb-3 flex flex-wrap items-center gap-1.5 text-[10px] text-foreground/70"
    >
      <Info size={11} aria-hidden="true" className="shrink-0" />
      <span>{t('jobs.locationFilterHint')}</span>
      {affected.map((e) => (
        <span
          key={e.id}
          className="rounded-full bg-muted px-1.5 py-0.5 font-medium text-foreground/70"
        >
          {t(`jobs.boards.${e.id}`, { defaultValue: e.id })}
        </span>
      ))}
    </div>
  );
}
