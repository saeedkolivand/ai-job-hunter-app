import { ChevronDown } from 'lucide-react';
import { useState } from 'react';

import type { DATE_FILTER_OPTIONS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, cn, Dropdown, LocationInput, NumberField } from '@ajh/ui';

import { AUTH_BENEFITS } from '../../constants';
import { REGIONS, type ScrapeFormState } from './constants';

interface Props {
  form: ScrapeFormState;
  scraping: boolean;
  boardConnected: boolean;
  onFormChange: (updates: Partial<ScrapeFormState>) => void;
  onGeocode: (query: string) => Promise<{ display: string }[]>;
}

const LABEL =
  'mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55';

export function ScrapeFilters({ form, scraping, boardConnected, onFormChange, onGeocode }: Props) {
  const { t } = useTranslation();
  // Location stays primary; the lower-value fields move behind an Advanced
  // disclosure (#35 — IA: low-priority controls don't crowd the form).
  const [showAdvanced, setShowAdvanced] = useState(false);

  return (
    <div className="@container mb-4 space-y-3">
      {/* Location — primary filter, always visible. */}
      <div>
        <label className={LABEL}>{t('jobs.location')}</label>
        <LocationInput
          value={form.location}
          onChange={(v) => onFormChange({ location: v })}
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
              className="w-full bg-white/[0.03] text-xs text-foreground disabled:opacity-50"
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
              className="w-full bg-white/[0.03] text-xs text-foreground disabled:opacity-50"
            />
          </div>

          {/* Indeed region */}
          {form.boards.includes('indeed') && (
            <div className="col-span-2">
              <label className={LABEL}>{t('jobs.region')}</label>
              <Dropdown
                options={REGIONS.map((r) => ({ value: r.value, label: t(r.labelKey) }))}
                value={form.locale}
                onChange={(value) => onFormChange({ locale: value })}
                disabled={scraping}
                placeholder={t('jobs.selectRegion')}
              />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
