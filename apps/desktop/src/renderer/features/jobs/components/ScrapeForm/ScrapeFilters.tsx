import { ChevronDown } from 'lucide-react';
import { useRef, useState } from 'react';

import { type DATE_FILTER_OPTIONS, WORK_TYPE_OPTIONS, type WorkTypeOption } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, cn, Dropdown, LocationInput, NumberField } from '@ajh/ui';

import { makeMultiSelectKeyHandler } from '@/hooks/use-roving-tabindex';

import { AUTH_BENEFITS } from '../../constants';
import type { ScrapeFormState } from './constants';

interface Props {
  form: ScrapeFormState;
  scraping: boolean;
  boardConnected: boolean;
  onFormChange: (updates: Partial<ScrapeFormState>) => void;
  onGeocode: (query: string) => Promise<{ display: string }[]>;
}

/** Toggle membership of `opt` in the array without mutation. */
function toggleWorkType(workTypes: WorkTypeOption[], opt: WorkTypeOption): WorkTypeOption[] {
  return workTypes.includes(opt) ? workTypes.filter((w) => w !== opt) : [...workTypes, opt];
}

const LABEL =
  'mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55';

export function ScrapeFilters({ form, scraping, boardConnected, onFormChange, onGeocode }: Props) {
  const { t } = useTranslation();
  // Location stays primary; the lower-value fields move behind an Advanced
  // disclosure (#35 — IA: low-priority controls don't crowd the form).
  const [showAdvanced, setShowAdvanced] = useState(false);
  const workTypeRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const focusedWorkTypeIdx = useRef<number>(0);

  return (
    <div className="@container mb-4 space-y-3">
      {/* Location — primary filter, always visible. */}
      <div>
        <label className={LABEL}>{t('jobs.location')}</label>
        <LocationInput
          value={form.location}
          // Freehand typing invalidates the previously PICKED suggestion, so drop
          // the whole structured payload it captured. `countryCode` is the part
          // that changes behavior today: a stale one searches the old market AND
          // suppresses the backend's geocode backfill (which only runs when it is
          // absent). lat/lon go with it for consistency — no board consumes them
          // yet, but they ride into the same `LocationSpec`, and coordinates from
          // a city the user just typed away from would make that spec incoherent.
          // Mirrors autopilot's StepTarget.
          onChange={(v) =>
            onFormChange({
              location: v,
              countryCode: undefined,
              latitude: undefined,
              longitude: undefined,
            })
          }
          onSelectSuggestion={(s) =>
            onFormChange({
              location: s.display,
              countryCode: s.countryCode ?? undefined,
              latitude: s.lat ?? undefined,
              longitude: s.lon ?? undefined,
            })
          }
          placeholder={t('jobs.locationPlaceholder')}
          disabled={scraping}
          onFetchSuggestions={onGeocode}
        />
      </div>

      <Button
        variant="unstyled"
        onClick={() => setShowAdvanced((v) => !v)}
        className="flex items-center gap-1.5 text-[11px] font-medium text-foreground/55 transition-colors hover:text-foreground/80"
      >
        <ChevronDown
          size={12}
          className={cn('transition-transform', showAdvanced && 'rotate-180')}
        />
        {t('jobs.advanced')}
      </Button>

      {showAdvanced && (
        <div className="grid grid-cols-1 gap-2 @xs:grid-cols-2">
          <div>
            <label className={LABEL}>{t('jobs.posted')}</label>
            <Dropdown
              options={[
                { value: '', label: t('jobs.anyTime') },
                ...(form.boards.some((b) => AUTH_BENEFITS.has(b)) && boardConnected
                  ? [
                      { value: '15m', label: t('jobs.past15m') },
                      { value: '30m', label: t('jobs.past30m') },
                      { value: '1h', label: t('jobs.past1h') },
                      { value: '2h', label: t('jobs.past2h') },
                      { value: '4h', label: t('jobs.past4h') },
                      { value: '8h', label: t('jobs.past8h') },
                    ]
                  : []),
                { value: '24h', label: t('jobs.past24h') },
                { value: 'week', label: t('jobs.pastWeek') },
                { value: 'month', label: t('jobs.pastMonth') },
              ]}
              value={form.dateFilter}
              onChange={(value) =>
                onFormChange({ dateFilter: value as '' | (typeof DATE_FILTER_OPTIONS)[number] })
              }
              disabled={scraping}
              placeholder={t('jobs.anyTime')}
            />
          </div>
          <div>
            <label className={LABEL}>{t('jobs.amount')}</label>
            <NumberField
              min={1}
              max={100}
              fallback={25}
              value={form.amount}
              onChange={(n) => onFormChange({ amount: n })}
              disabled={scraping}
              className="w-full bg-field shadow-none text-xs text-foreground disabled:opacity-50"
            />
          </div>
          <div>
            <label className={LABEL}>{t('jobs.radius')}</label>
            <NumberField
              min={0}
              max={200}
              fallback={0}
              value={form.radiusKm}
              onChange={(n) => onFormChange({ radiusKm: n })}
              disabled={scraping}
              className="w-full bg-field shadow-none text-xs text-foreground disabled:opacity-50"
            />
          </div>
          <div>
            <label className={LABEL}>{t('jobs.workType.label')}</label>
            {/* Multi-select set, not a Dropdown — a Dropdown can't express a set.
                Empty = any, all three = all (byte-equivalent to no filter on
                every board that validates the param). Mirrors the board picker
                idiom in ScrapeForm/index.tsx. */}
            <div
              data-testid={TEST_IDS.jobs.workTypeFilter}
              role="group"
              aria-label={t('jobs.workType.label')}
              className="flex flex-wrap gap-1.5"
              onKeyDown={
                scraping
                  ? undefined
                  : makeMultiSelectKeyHandler(
                      WORK_TYPE_OPTIONS.length,
                      focusedWorkTypeIdx,
                      workTypeRefs,
                      (idx) => {
                        const opt = WORK_TYPE_OPTIONS[idx];
                        if (opt !== undefined)
                          onFormChange({ workTypes: toggleWorkType(form.workTypes, opt) });
                      }
                    )
              }
            >
              {WORK_TYPE_OPTIONS.map((opt, i) => {
                const active = form.workTypes.includes(opt);
                return (
                  <Button
                    key={opt}
                    ref={(el) => {
                      workTypeRefs.current[i] = el;
                    }}
                    aria-pressed={active}
                    tabIndex={i === focusedWorkTypeIdx.current ? 0 : -1}
                    variant="ghost"
                    disabled={scraping}
                    onClick={() => {
                      focusedWorkTypeIdx.current = i;
                      onFormChange({ workTypes: toggleWorkType(form.workTypes, opt) });
                    }}
                    className={cn(
                      'rounded-lg px-2.5 py-1 text-[11px] transition-all',
                      active
                        ? 'bg-brand/20 text-brand-soft ring-1 ring-brand/40'
                        : 'bg-card border border-[var(--border-clear)] text-foreground/50 hover:bg-muted hover:text-foreground/80',
                      'disabled:cursor-not-allowed disabled:opacity-40'
                    )}
                  >
                    {t(`jobs.workType.${opt}`)}
                  </Button>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
