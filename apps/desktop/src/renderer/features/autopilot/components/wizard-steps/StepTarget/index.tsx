import { Globe } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { Controller, useFormContext, useWatch } from 'react-hook-form';

import {
  AGGREGATOR_BOARD_ID,
  type BoardCatalogEntry,
  PROVIDER_SLOTS,
  WORK_TYPE_OPTIONS,
} from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Alert, Button, cn, Dropdown, Input, LocationInput, NumberField } from '@ajh/ui';

import { LocationFilterNote, WorkTypeFilterNote } from '@/components/scrape/LocationFilterNote';
import { SeededCompaniesNote } from '@/components/scrape/SeededCompaniesNote';
import type { Prefilled, WizardState } from '@/features/autopilot/types';
import { makeMultiSelectKeyHandler } from '@/hooks/use-roving-tabindex';
import { regionName } from '@/lib/region-name';
import { useAppClient } from '@/providers/AppClientProvider';
import { useHasProviderKey } from '@/services/use-ai-provider';
import { useBoardsCatalog } from '@/services/use-boards';

import { PrefilledBadge } from '../PrefilledBadge';
import { WatchedCompaniesField } from '../WatchedCompaniesField';
import { WizardField } from '../WizardField';

const fieldCls = 'h-9 w-full text-xs shadow-none';

// The backend `AutopilotTargetSchema.pages` range, mirrored by `autopilotWizardSchema`.
// Bound once here so the NumberField's own clamp and the inert-value normalization
// below cannot drift apart — a normalization that landed outside the schema range
// would re-create the very block it exists to clear.
const PAGES_MIN = 1;
const PAGES_MAX = 10;
const PAGES_FALLBACK = 2;

interface StepTargetProps {
  prefilled: Prefilled;
}

export function StepTarget({ prefilled }: StepTargetProps) {
  const { t, i18n } = useTranslation();
  const api = useAppClient();
  const { control, getValues, setValue } = useFormContext<WizardState>();
  const boards = useWatch({ control, name: 'boards' });
  // Country derived from the picked location suggestion — surfaced inline so the
  // user SEES which market the autopilot will search (vs. the silent save-time
  // backfill, now only a legacy fallback). Cleared by editing the location.
  const countryCode = useWatch({ control, name: 'countryCode' });
  // Location text — drives the honest "location filtered locally" board hint.
  const location = useWatch({ control, name: 'location' });
  // Work-type selection — drives the honest "work type filtered locally" board hint.
  const workTypes = useWatch({ control, name: 'workTypes' });

  const boardRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const focusedBoardIdx = useRef<number>(0);

  const { data: catalogRaw, isLoading: catalogLoading } = useBoardsCatalog();
  const listedBoards: BoardCatalogEntry[] = (catalogRaw ?? []).filter((e) => e.listed);

  // Selected boards + whether a location is set — feeds the location hint below.
  const selectedListedBoards = listedBoards.filter((e) => boards.includes(e.id));
  const hasLocation = (location ?? '').trim().length > 0;

  // Normalize: ensure every persisted board id still exists in the catalog.
  // Mirror ScrapeForm normalization guard — prevents an infinite re-render loop
  // by only calling onChange when the normalized set actually differs.
  useEffect(() => {
    if (catalogLoading || listedBoards.length === 0) return;
    const listedIds = new Set(listedBoards.map((e) => e.id));
    const valid = boards.filter((id) => listedIds.has(id));
    const needsUpdate = valid.length !== boards.length || boards.length === 0;
    if (!needsUpdate) return;
    const fallback = listedBoards[0]?.id ?? '';
    setValue('boards', valid.length > 0 ? valid : fallback ? [fallback] : []);
  }, [catalogLoading, listedBoards, boards, setValue]);

  // Aggregator key hint — shown when aggregator is selected but Adzuna keys absent.
  const selectedSet = new Set(boards);
  const aggregatorSelected = selectedSet.has(AGGREGATOR_BOARD_ID);
  const { data: adzunaIdData } = useHasProviderKey(PROVIDER_SLOTS.adzunaAppId, aggregatorSelected);
  const { data: adzunaKeyData } = useHasProviderKey(
    PROVIDER_SLOTS.adzunaAppKey,
    aggregatorSelected
  );
  const showAggregatorKeyHint = aggregatorSelected && !(adzunaIdData?.has && adzunaKeyData?.has);

  // The page budget is an INERT knob when the aggregator is the only target: the
  // aggregator board never reads `pages`. Its upstream APIs are metered (Adzuna
  // bills a daily call quota), so what it may spend rides a separate, explicit
  // budget that a SCHEDULED run deliberately leaves unset — an autopilot run
  // therefore costs exactly one upstream call no matter what is typed here.
  // Showing an editable field that changes nothing is dishonest, so disable it
  // and say why. A MIXED selection keeps it live — every other board honours it.
  // Counted on the deduplicated SET (like `aggregatorSelected` above): a
  // persisted `['aggregator','aggregator']` is still an aggregator-only target,
  // and `boards.length` would call it mixed and leave a dead knob live.
  const pagesInert = aggregatorSelected && selectedSet.size === 1;

  // `pages` stays schema-validated while inert, so a persisted out-of-range or
  // non-integer value would block "Next" on a field the user cannot reach — the
  // control is disabled, so there is no way to fix what the wizard is complaining
  // about. Normalize it into the schema's range instead (the same round+clamp the
  // NumberField applies on blur). Safe because the value is INERT here: the
  // aggregator ignores it, and re-adding a pages-aware board re-enables the field
  // with a sane, editable number rather than a permanently invalid one.
  useEffect(() => {
    if (!pagesInert) return;
    const current = getValues('pages');
    const normalized = Number.isFinite(current)
      ? Math.min(PAGES_MAX, Math.max(PAGES_MIN, Math.round(current)))
      : PAGES_FALLBACK;
    if (normalized !== current) {
      setValue('pages', normalized, { shouldDirty: true, shouldValidate: true });
    }
  }, [pagesInert, getValues, setValue]);

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.wizard.target.title')}
        </p>
        <p className="text-xs text-foreground/35 mt-0.5">{t('autopilot.wizard.target.subtitle')}</p>
      </div>

      <Controller
        control={control}
        name="name"
        render={({ field, fieldState }) => (
          <WizardField
            label={t('autopilot.wizard.target.name')}
            htmlFor="autopilot-name"
            error={fieldState.error?.message ? t(fieldState.error.message) : undefined}
          >
            <Input
              id="autopilot-name"
              variant="default"
              className={fieldCls}
              placeholder={t('autopilot.wizard.target.namePlaceholder')}
              value={field.value}
              onChange={field.onChange}
              onBlur={field.onBlur}
              aria-invalid={!!fieldState.error}
            />
          </WizardField>
        )}
      />

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
                <WorkTypeFilterNote boards={selectedListedBoards} active={workTypes.length > 0} />
              </div>

              {/* Seeded-companies disclosure — names the curated companies a
                  company-scoped ATS board (Greenhouse/Lever/Ashby/…) will query (#621) */}
              <SeededCompaniesNote boards={selectedListedBoards} />
            </WizardField>
          );
        }}
      />

      {/* Watched-companies target (ADR-030 §e) — resolve the user's starred
          companies at run time instead of the curated seed. */}
      <WatchedCompaniesField />

      <Controller
        control={control}
        name="workTypes"
        render={({ field }) => {
          const sel = new Set(field.value);
          const toggle = (opt: (typeof WORK_TYPE_OPTIONS)[number]) => {
            field.onChange(
              sel.has(opt) ? field.value.filter((w) => w !== opt) : [...field.value, opt]
            );
          };
          return (
            <WizardField
              label={t('autopilot.wizard.target.workType')}
              // Empty set silently means "any" — three neutral, identically
              // unselected buttons read as broken/unset otherwise. Same
              // "Any time"-style visible microcopy idiom as the Posted
              // Dropdown's own empty state.
              hint={field.value.length === 0 ? t('jobs.workType.any') : undefined}
            >
              {/* Multi-select set, not a Dropdown — a Dropdown can't express a
                  set. Empty = any, all three = all. Mirrors the board picker
                  above and ScrapeForm's manual-search control — including its
                  visual language (same selected/unselected classes) and its
                  flex-wrap (not a fixed grid column, which can clip "Vor
                  Ort"). Plain tab stops, not roving tabindex: that pattern
                  earns its keep on the ~26-item board picker above, but a
                  3-item set has no efficiency win from it and it breaks the
                  standard "Tab moves to the next toggle" expectation. */}
              <div
                role="group"
                aria-label={t('autopilot.wizard.target.workType')}
                className="flex flex-wrap gap-1.5"
              >
                {WORK_TYPE_OPTIONS.map((opt) => {
                  const active = sel.has(opt);
                  return (
                    <Button
                      key={opt}
                      aria-pressed={active}
                      variant="ghost"
                      onClick={() => toggle(opt)}
                      className={cn(
                        'rounded-lg px-2.5 py-1 text-[11px] transition-all',
                        active
                          ? 'bg-brand/20 text-brand-soft ring-1 ring-brand/40'
                          : 'bg-card border border-[var(--border-clear)] text-foreground/50 hover:bg-muted hover:text-foreground/80'
                      )}
                    >
                      {t(`jobs.workType.${opt}`)}
                    </Button>
                  );
                })}
              </div>
            </WizardField>
          );
        }}
      />

      <div className="grid grid-cols-1 gap-3 @xs:grid-cols-2">
        <Controller
          control={control}
          name="query"
          render={({ field, fieldState }) => (
            <WizardField
              label={t('autopilot.wizard.target.query')}
              htmlFor="autopilot-query"
              error={fieldState.error?.message ? t(fieldState.error.message) : undefined}
            >
              <Input
                id="autopilot-query"
                variant="default"
                className={fieldCls}
                placeholder={t('autopilot.wizard.target.queryPlaceholder')}
                value={field.value}
                onChange={field.onChange}
                onBlur={field.onBlur}
                aria-invalid={!!fieldState.error}
              />
            </WizardField>
          )}
        />
        <Controller
          control={control}
          name="location"
          render={({ field }) => (
            <WizardField
              label={t('autopilot.wizard.target.location')}
              hint={t('autopilot.wizard.target.locationOptional')}
            >
              <div className="space-y-1.5">
                <LocationInput
                  value={field.value}
                  onChange={(v) => {
                    field.onChange(v);
                    setValue('countryCode', undefined, { shouldDirty: true });
                  }}
                  onSelectSuggestion={(s) => {
                    field.onChange(s.display);
                    setValue('countryCode', s.countryCode ?? undefined, { shouldDirty: true });
                  }}
                  placeholder={t('autopilot.wizard.target.locationPlaceholder')}
                  onFetchSuggestions={(q) => api.geocode.suggest(q)}
                />
                {countryCode && (
                  <p className="flex items-center gap-1 text-[10px] text-foreground/45">
                    <Globe size={10} aria-hidden />
                    {t('autopilot.wizard.target.countryResolved', {
                      country: regionName(countryCode, i18n.language),
                    })}
                  </p>
                )}
                {prefilled.location && (
                  <PrefilledBadge field={t('autopilot.wizard.target.fromLocationSettings')} />
                )}
              </div>
            </WizardField>
          )}
        />
      </div>

      <div className="grid grid-cols-1 gap-3 @xs:grid-cols-2">
        <Controller
          control={control}
          name="pages"
          render={({ field, fieldState }) => (
            <WizardField
              label={t('autopilot.wizard.target.pages')}
              hint={
                pagesInert
                  ? t('autopilot.wizard.target.pagesAggregatorOnly')
                  : t('autopilot.wizard.target.pagesHint')
              }
              htmlFor="autopilot-pages"
              error={fieldState.error?.message ? t(fieldState.error.message) : undefined}
            >
              <NumberField
                id="autopilot-pages"
                min={PAGES_MIN}
                max={PAGES_MAX}
                fallback={PAGES_FALLBACK}
                variant="default"
                className={cn(fieldCls, 'disabled:opacity-40 disabled:cursor-not-allowed')}
                disabled={pagesInert}
                value={field.value}
                onChange={(n) => field.onChange(n)}
                aria-invalid={!!fieldState.error}
                onBlur={() => {
                  field.onBlur();
                  // NumberField clamps to [min, max] on blur but never rounds, so a
                  // typed "2.5" would reach the schema's .int() as invalid and make
                  // the final Create button a silent no-op. Fold it to a whole page
                  // here — on blur only, so the buffer isn't fought mid-typing.
                  setValue('pages', Math.round(getValues('pages')), {
                    shouldDirty: true,
                    shouldValidate: true,
                  });
                }}
              />
            </WizardField>
          )}
        />
        <Controller
          control={control}
          name="dateFilter"
          render={({ field }) => (
            <WizardField label={t('autopilot.wizard.target.postedWithin')}>
              <Dropdown
                options={[
                  { value: '', label: t('autopilot.wizard.target.anyTime') },
                  { value: '24h', label: t('autopilot.wizard.target.last24h') },
                  { value: 'week', label: t('autopilot.wizard.target.lastWeek') },
                  { value: 'month', label: t('autopilot.wizard.target.lastMonth') },
                ]}
                value={field.value}
                onChange={field.onChange}
                placeholder={t('autopilot.wizard.target.anyTime')}
                tone="field"
                className="h-9 shadow-none"
              />
            </WizardField>
          )}
        />
      </div>
    </div>
  );
}
