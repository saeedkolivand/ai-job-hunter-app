import { Globe } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { Controller, useFormContext, useWatch } from 'react-hook-form';

import { AGGREGATOR_BOARD_ID, type BoardCatalogEntry, PROVIDER_SLOTS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Alert, Button, cn, Dropdown, Input, LocationInput, NumberField } from '@ajh/ui';

import { LocationFilterNote } from '@/components/scrape/LocationFilterNote';
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

interface StepTargetProps {
  prefilled: Prefilled;
}

export function StepTarget({ prefilled }: StepTargetProps) {
  const { t, i18n } = useTranslation();
  const api = useAppClient();
  const { control, setValue } = useFormContext<WizardState>();
  const boards = useWatch({ control, name: 'boards' });
  // Country derived from the picked location suggestion — surfaced inline so the
  // user SEES which market the autopilot will search (vs. the silent save-time
  // backfill, now only a legacy fallback). Cleared by editing the location.
  const countryCode = useWatch({ control, name: 'countryCode' });
  // Location text — drives the honest "location filtered locally" board hint.
  const location = useWatch({ control, name: 'location' });

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
                      {t(`jobs.boards.${id}`)}
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
          name="amount"
          render={({ field }) => (
            <WizardField label={t('autopilot.wizard.target.items')}>
              <NumberField
                min={1}
                max={500}
                fallback={25}
                variant="default"
                className={fieldCls}
                value={field.value}
                onChange={(n) => field.onChange(n)}
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
