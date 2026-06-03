import type { DATE_FILTER_OPTIONS } from '@ajh/shared';
import { Input, LocationInput, SelectDropdown } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import { AUTH_BENEFITS, REGIONS, type ScrapeFormState } from './constants';

interface Props {
  form: ScrapeFormState;
  scraping: boolean;
  boardConnected: boolean;
  onFormChange: (updates: Partial<ScrapeFormState>) => void;
  onGeocode: (query: string) => Promise<{ display: string }[]>;
}

export function ScrapeFilters({ form, scraping, boardConnected, onFormChange, onGeocode }: Props) {
  const { t } = useTranslation();

  return (
    <div className="mb-4 grid grid-cols-4 gap-2">
      <div className="col-span-2">
        <label className="mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('jobs.location')}
        </label>
        <LocationInput
          value={form.location}
          onChange={(v) => onFormChange({ location: v })}
          placeholder={t('jobs.locationPlaceholder')}
          disabled={scraping}
          onFetchSuggestions={onGeocode}
        />
      </div>
      <div>
        <label className="mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('jobs.posted')}
        </label>
        <SelectDropdown
          options={[
            { value: '', label: t('jobs.anyTime') },
            ...(AUTH_BENEFITS.has(form.board) && boardConnected
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
        <label className="mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('jobs.pages')}
        </label>
        <Input
          type="number"
          min="1"
          max="20"
          value={form.pages}
          onChange={(e) => onFormChange({ pages: parseInt(e.target.value) || 1 })}
          disabled={scraping}
          className="w-full bg-white/[0.03] text-xs text-foreground disabled:opacity-50"
        />
      </div>

      {/* Indeed region */}
      {form.board === 'indeed' && (
        <div className="col-span-4">
          <label className="mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
            {t('jobs.region')}
          </label>
          <SelectDropdown
            options={REGIONS.map((r) => ({ value: r.value, label: t(r.labelKey) }))}
            value={form.locale}
            onChange={(value) => onFormChange({ locale: value })}
            disabled={scraping}
            placeholder={t('jobs.selectRegion')}
          />
        </div>
      )}
    </div>
  );
}
