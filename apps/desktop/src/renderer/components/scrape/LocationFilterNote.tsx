import { Info } from 'lucide-react';

import type { BoardCatalogEntry } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';

interface FilterCapabilityNoteProps {
  /** The currently-selected listed boards (as catalog entries). */
  boards: readonly BoardCatalogEntry[];
  /** True once the filter this hint describes is actually in effect — the
   *  hint is meaningless otherwise (no location typed / no work type picked). */
  active: boolean;
  /** i18n key for the lead-in text (chip mode), or for the whole sentence
   *  (summary mode — interpolated with `{{count}}`/`{{total}}`). */
  hintKey: string;
  /** Reads the board's support flag for this capability off the catalog entry. */
  supports: (board: BoardCatalogEntry) => boolean | undefined;
  /**
   * Collapse the affected-board list into a count/summary sentence instead of
   * one chip per board. Location has only 4 non-supporting boards out of 26 —
   * naming them is the point. Work type has 25 of 26 — naming them all reads
   * as noise, not a hint, so it uses the summary sentence instead.
   */
  summary?: boolean;
}

/**
 * Shared body for the honest per-board picker hint (PR F, job-search trust
 * program; generalised to any board capability flag — location first, work
 * type since). While `active`, names the SELECTED boards that do NOT narrow
 * results by this capability on their side (`supports` falsy). The scrape
 * engine post-filters those boards' results on-device (dropping only clear
 * mismatches), so this tells the user upfront which boards "ignore" the
 * filter upstream — informational and muted, never a warning.
 *
 * Not exported — {@link LocationFilterNote} and {@link WorkTypeFilterNote} are
 * thin, capability-specific wrappers over this one body.
 */
function FilterCapabilityNote({
  boards,
  active,
  hintKey,
  supports,
  summary,
}: FilterCapabilityNoteProps) {
  const { t } = useTranslation();
  if (!active) return null;

  // Optional flag: an absent / older payload reads as "does not support this
  // capability" — matching the contract semantics and the engine's
  // conservative post-filter.
  const affected = boards.filter((b) => !supports(b));
  if (affected.length === 0) return null;

  if (summary) {
    return (
      <div role="note" className="mb-3 flex items-center gap-1.5 text-[10px] text-foreground/70">
        <Info size={11} aria-hidden="true" className="shrink-0" />
        <span>{t(hintKey, { count: affected.length, total: boards.length })}</span>
      </div>
    );
  }

  return (
    <div
      role="note"
      className="mb-3 flex flex-wrap items-center gap-1.5 text-[10px] text-foreground/70"
    >
      <Info size={11} aria-hidden="true" className="shrink-0" />
      <span>{t(hintKey)}</span>
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

interface LocationFilterNoteProps {
  /** The currently-selected listed boards (as catalog entries). */
  boards: readonly BoardCatalogEntry[];
  /** True once the user has entered a location — the hint is meaningless otherwise. */
  hasLocation: boolean;
}

/**
 * Location variant of {@link FilterCapabilityNote}. Shared by the jobs manual
 * picker and the autopilot wizard, so it lives outside both feature dirs (no
 * cross-feature import), mirroring `BoardSummaryChips`.
 */
export function LocationFilterNote({ boards, hasLocation }: LocationFilterNoteProps) {
  return (
    <FilterCapabilityNote
      boards={boards}
      active={hasLocation}
      hintKey="jobs.locationFilterHint"
      supports={(b) => b.supportsLocation}
    />
  );
}

interface WorkTypeFilterNoteProps {
  /** The currently-selected listed boards (as catalog entries). */
  boards: readonly BoardCatalogEntry[];
  /** True once the user has picked at least one work type — the hint is
   *  meaningless otherwise. */
  active: boolean;
}

/**
 * Work-type variant of {@link FilterCapabilityNote} (work-type filter feature).
 * Only `smartrecruiters` validates the filter server-side today, so a
 * chip-per-board list (the location variant's presentation) would name
 * almost the entire selection — noise, not a hint. Collapses to a count
 * summary sentence instead (`summary`).
 */
export function WorkTypeFilterNote({ boards, active }: WorkTypeFilterNoteProps) {
  return (
    <FilterCapabilityNote
      boards={boards}
      active={active}
      hintKey="jobs.workType.filterSummary"
      supports={(b) => b.supportsWorkType}
      summary
    />
  );
}
