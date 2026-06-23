import { Info } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { Controller, useFormContext, useWatch } from 'react-hook-form';

import { AGGREGATOR_BOARD_ID, type BoardCatalogEntry, PROVIDER_SLOTS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, cn, Dropdown, Input, LocationInput, NumberField } from '@ajh/ui';

import type { Prefilled, WizardState } from '@/features/autopilot/types';
import { makeMultiSelectKeyHandler } from '@/hooks/use-roving-tabindex';
import { useAppClient } from '@/providers/AppClientProvider';
import { useHasProviderKey } from '@/services/use-ai-provider';
import { useBoardsCatalog } from '@/services/use-boards';

import { ComingSoonBadge } from '../ComingSoonBadge';
import { PrefilledBadge } from '../PrefilledBadge';
import { WizardField } from '../WizardField';

// Matches @ajh/ui LocationInput trigger (h-9, same border & bg) so text inputs
// sit flush with sibling controls on the same row. The Dropdown uses tone="field"
// for matching border/bg, plus className="h-9 shadow-none" for height alignment.
const inputCls =
  'w-full h-9 rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 text-xs text-foreground/80 placeholder:text-foreground/25 outline-none focus:border-brand/40 transition-colors';

interface StepTargetProps {
  prefilled: Prefilled;
}

export function StepTarget({ prefilled }: StepTargetProps) {
  const { t } = useTranslation();
  const api = useAppClient();
  const { control, setValue } = useFormContext<WizardState>();
  // Disabled "coming soon" control — display the current value without binding.
  const workType = useWatch({ control, name: 'workType' });
  const boards = useWatch({ control, name: 'boards' });

  const boardRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const focusedBoardIdx = useRef<number>(0);

  const { data: catalogRaw, isLoading: catalogLoading } = useBoardsCatalog();
  const listedBoards: BoardCatalogEntry[] = (catalogRaw ?? []).filter((e) => e.listed);

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
              variant="unstyled"
              className={inputCls}
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
                          : 'border-white/[0.06] text-foreground/40 hover:border-white/10 hover:text-foreground/65'
                      )}
                    >
                      {t(`jobs.boards.${id}`)}
                    </Button>
                  );
                })}
              </div>

              {/* Aggregator key hint — mirrors ScrapeForm */}
              {showAggregatorKeyHint && (
                <p
                  role="status"
                  className="mt-2 flex items-center gap-1.5 text-[11px] text-amber-400/70"
                >
                  <Info size={11} aria-hidden="true" />
                  {t('jobs.aggregatorKeyHint')}
                </p>
              )}
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
                variant="unstyled"
                className={inputCls}
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
                  onChange={field.onChange}
                  placeholder={t('autopilot.wizard.target.locationPlaceholder')}
                  onFetchSuggestions={(q) => api.geocode.suggest(q)}
                />
                {prefilled.location && (
                  <PrefilledBadge field={t('autopilot.wizard.target.fromLocationSettings')} />
                )}
              </div>
            </WizardField>
          )}
        />
      </div>

      <WizardField label={t('autopilot.wizard.target.workType')} badge={<ComingSoonBadge />}>
        <div className="grid grid-cols-2 gap-1.5 @sm:grid-cols-4">
          {(['any', 'remote', 'hybrid', 'on-site'] as const).map((opt) => (
            <Button
              key={opt}
              disabled
              className={cn(
                'rounded-lg border px-2 py-1.5 text-[10px] font-medium capitalize transition-all h-auto',
                workType === opt
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-white/[0.06] text-foreground/40'
              )}
            >
              {opt}
            </Button>
          ))}
        </div>
      </WizardField>

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
                variant="unstyled"
                className={inputCls}
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
