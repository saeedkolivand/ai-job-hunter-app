import { useRef } from 'react';
import { Controller, useFormContext } from 'react-hook-form';

import type { BoardCatalogEntry } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Alert, Button, cn } from '@ajh/ui';

import { LocationFilterNote, WorkTypeFilterNote } from '@/components/scrape/LocationFilterNote';
import { SeededCompaniesNote } from '@/components/scrape/SeededCompaniesNote';
import type { WizardState } from '@/features/autopilot/types';
import { makeMultiSelectKeyHandler } from '@/hooks/use-roving-tabindex';

import { WizardField } from '../WizardField';

interface Props {
  listedBoards: BoardCatalogEntry[];
  selectedListedBoards: BoardCatalogEntry[];
  hasLocation: boolean;
  workTypeActive: boolean;
  showAggregatorKeyHint: boolean;
}

/** Roving-tabindex multi-select of listed boards, plus the honest per-board disclosures. */
export function BoardPicker({
  listedBoards,
  selectedListedBoards,
  hasLocation,
  workTypeActive,
  showAggregatorKeyHint,
}: Props) {
  const { t } = useTranslation();
  const { control } = useFormContext<WizardState>();
  const boardRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const focusedBoardIdx = useRef<number>(0);

  return (
    <Controller
      control={control}
      name="boards"
      render={({ field }) => {
        const sel = new Set(field.value);
        const toggle = (b: string) => {
          const next = sel.has(b) ? field.value.filter((id) => id !== b) : [...field.value, b];
          // Always keep at least one board selected.
          if (next.length > 0) field.onChange(next);
        };
        return (
          <WizardField label={t('autopilot.wizard.target.board')}>
            <div
              role="group"
              aria-label={t('autopilot.wizard.target.board')}
              className="grid grid-cols-2 gap-1.5 max-h-28 overflow-y-auto pr-1 @sm:grid-cols-4"
              onKeyDown={makeMultiSelectKeyHandler(
                listedBoards.length,
                focusedBoardIdx,
                boardRefs,
                (idx) => {
                  const b = listedBoards[idx]?.id;
                  if (b !== undefined) toggle(b);
                }
              )}
            >
              {listedBoards.map(({ id }, i) => {
                const active = sel.has(id);
                return (
                  <Button
                    key={id}
                    ref={(el) => {
                      boardRefs.current[i] = el;
                    }}
                    aria-pressed={active}
                    tabIndex={i === focusedBoardIdx.current ? 0 : -1}
                    onClick={() => {
                      focusedBoardIdx.current = i;
                      toggle(id);
                    }}
                    className={cn(
                      'rounded-lg border px-2 py-1.5 text-[10px] font-medium capitalize transition-all h-auto',
                      active
                        ? 'border-brand/40 bg-brand/10 text-brand-soft'
                        : 'border-[var(--border-clear)] text-foreground/40 hover:bg-muted hover:text-foreground/65'
                    )}
                  >
                    {t(`jobs.boards.${id}`, { defaultValue: id })}
                  </Button>
                );
              })}
            </div>

            {/* Aggregator key hint — mirrors ScrapeForm */}
            {showAggregatorKeyHint && (
              <div className="mt-2">
                <Alert type="warning" showIcon message={t('jobs.aggregatorKeyHint')} />
              </div>
            )}

            {/* Honest location hint — mirrors ScrapeForm */}
            <div className="mt-2 empty:mt-0">
              <LocationFilterNote boards={selectedListedBoards} hasLocation={hasLocation} />
            </div>

            {/* Same honesty disclosure for work type — mirrors ScrapeForm */}
            <div className="mt-2 empty:mt-0">
              <WorkTypeFilterNote boards={selectedListedBoards} active={workTypeActive} />
            </div>

            {/* Seeded-companies disclosure — names the curated companies a
                company-scoped ATS board (Greenhouse/Lever/Ashby/…) will query (#621) */}
            <SeededCompaniesNote boards={selectedListedBoards} />
          </WizardField>
        );
      }}
    />
  );
}
