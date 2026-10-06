import { useRef } from 'react';

import type { BoardCatalogEntry } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, CardSkeleton, cn } from '@ajh/ui';

import { makeMultiSelectKeyHandler } from '@/hooks/use-roving-tabindex';

import type { ScrapeFormState } from './constants';

interface BoardPickerProps {
  listedBoards: BoardCatalogEntry[];
  catalogLoading: boolean;
  selected: string[];
  scraping: boolean;
  /** "3 selected" — i18next picks the plural form. */
  countLabel: string;
  onFormChange: (updates: Partial<ScrapeFormState>) => void;
}

/** Toggle membership of `id` in the array without mutation. */
function toggleBoard(boards: string[], id: string): string[] {
  return boards.includes(id) ? boards.filter((b) => b !== id) : [...boards, id];
}

const smallActionCls =
  'h-auto rounded px-1.5 py-1 text-[10px] text-foreground/50 hover:text-foreground/80 disabled:opacity-40';

/** Board picker — multi-select toggle group with select-all / clear. */
export function BoardPicker({
  listedBoards,
  catalogLoading,
  selected,
  scraping,
  countLabel,
  onFormChange,
}: BoardPickerProps) {
  const { t } = useTranslation();
  const boardRefs = useRef<(HTMLButtonElement | null)[]>([]);
  // Tracks keyboard-focus position independently of the selection set (multi-select pattern).
  const focusedBoardIdx = useRef<number>(0);
  const selectedSet = new Set(selected);
  const allSelected = listedBoards.length > 0 && listedBoards.every((e) => selectedSet.has(e.id));

  const handleSelectAll = () => {
    onFormChange({ boards: listedBoards.map((e) => e.id) });
  };
  const handleClear = () => {
    // Always keep at least one; clear to the first listed board.
    const first = listedBoards[0]?.id;
    if (first) onFormChange({ boards: [first] });
  };

  return (
    <div className="mb-4">
      <div className="mb-2 flex items-center gap-2">
        <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('jobs.board')}
        </span>
        {!catalogLoading && listedBoards.length > 0 && (
          <>
            <span
              aria-live="polite"
              aria-atomic="true"
              className="rounded-full bg-brand/20 px-1.5 py-px text-[10px] font-medium text-brand-soft"
            >
              {countLabel}
            </span>
            <div className="ml-auto flex items-center gap-1">
              <Button
                variant="ghost"
                disabled={scraping || allSelected}
                onClick={handleSelectAll}
                className={smallActionCls}
              >
                {t('jobs.selectAll')}
              </Button>
              <Button
                variant="ghost"
                disabled={scraping || selected.length <= 1}
                onClick={handleClear}
                className={smallActionCls}
              >
                {t('jobs.clearBoards')}
              </Button>
            </div>
          </>
        )}
      </div>
      {catalogLoading ? (
        <CardSkeleton className="h-8 w-full" />
      ) : (
        <div
          role="group"
          aria-label={t('jobs.board')}
          className="flex flex-wrap gap-1.5"
          onKeyDown={
            scraping
              ? undefined
              : makeMultiSelectKeyHandler(
                  listedBoards.length,
                  focusedBoardIdx,
                  boardRefs,
                  (idx) => {
                    const id = listedBoards[idx]?.id;
                    if (!id) return;
                    // Prevent deselecting the last board.
                    if (selectedSet.has(id) && selected.length === 1) return;
                    onFormChange({ boards: toggleBoard(selected, id) });
                  }
                )
          }
        >
          {listedBoards.map(({ id }, i) => {
            const active = selectedSet.has(id);
            return (
              <Button
                key={id}
                ref={(el) => {
                  boardRefs.current[i] = el;
                }}
                aria-pressed={active}
                tabIndex={i === focusedBoardIdx.current ? 0 : -1}
                variant="ghost"
                disabled={scraping}
                onClick={() => {
                  // Prevent deselecting the last board.
                  if (active && selected.length === 1) return;
                  focusedBoardIdx.current = i;
                  onFormChange({ boards: toggleBoard(selected, id) });
                }}
                className={cn(
                  'rounded-lg px-2.5 py-1 text-[11px] transition-all',
                  active
                    ? 'bg-brand/20 text-brand-soft ring-1 ring-brand/40'
                    : 'bg-card border border-[var(--border-clear)] text-foreground/50 hover:bg-muted hover:text-foreground/80',
                  'disabled:cursor-not-allowed disabled:opacity-40'
                )}
              >
                {t(`jobs.boards.${id}`, { defaultValue: id })}
              </Button>
            );
          })}
        </div>
      )}
    </div>
  );
}
