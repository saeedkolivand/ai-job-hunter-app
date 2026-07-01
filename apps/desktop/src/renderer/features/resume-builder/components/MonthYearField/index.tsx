import { useMemo } from 'react';

import { useTranslation } from '@ajh/translations';
import { Dropdown } from '@ajh/ui';

interface MonthYearFieldProps {
  /** Serialized as "MMM YYYY" (e.g. "Jan 2020"). Empty ⇒ both dropdowns unset. */
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  /** When true, renders a read-only "Present" state instead of the dropdowns. */
  present?: boolean;
  /**
   * Forwarded to the first (month) dropdown's trigger so an external
   * `<label htmlFor>` (e.g. WizardField's injected id) resolves to a real control.
   */
  id?: string;
}

const MIN_YEAR = 1960;

/** Locale-aware short month labels (Jan…Dec). Stable English keys, localized labels. */
function useMonthOptions() {
  const { i18n } = useTranslation();
  return useMemo(() => {
    const fmt = new Intl.DateTimeFormat(i18n.language || undefined, { month: 'short' });
    // Stable English key (canonical serialized token) + localized display label.
    const keyFmt = new Intl.DateTimeFormat('en-US', { month: 'short' });
    return Array.from({ length: 12 }, (_, m) => {
      const date = new Date(2000, m, 1);
      return { value: keyFmt.format(date), label: fmt.format(date) };
    });
  }, [i18n.language]);
}

function useYearOptions() {
  return useMemo(() => {
    const max = new Date().getFullYear() + 1;
    const years: { value: string; label: string }[] = [];
    for (let y = max; y >= MIN_YEAR; y -= 1) years.push({ value: String(y), label: String(y) });
    return years;
  }, []);
}

/**
 * Month + year picker composed from two {@link Dropdown}s. Parses/serializes
 * its value as "MMM YYYY" (English month token, e.g. "Jan 2020"); a value is only
 * emitted once both month and year are chosen, and either can be cleared.
 */
export function MonthYearField({ value, onChange, disabled, present, id }: MonthYearFieldProps) {
  const { t } = useTranslation();
  const monthOptions = useMonthOptions();
  const yearOptions = useYearOptions();

  if (present) {
    return (
      <div className="rounded-lg border border-[var(--border-soft)] bg-foreground/[0.03] px-3 py-2 text-xs text-foreground/45">
        {t('build.experience.present')}
      </div>
    );
  }

  const [rawMonth = '', rawYear = ''] = value.trim().split(/\s+/);
  const month = monthOptions.some((o) => o.value === rawMonth) ? rawMonth : '';
  const year = /^\d{4}$/.test(rawYear) ? rawYear : '';

  const emit = (nextMonth: string, nextYear: string) => {
    onChange(nextMonth && nextYear ? `${nextMonth} ${nextYear}` : '');
  };

  return (
    <div className="grid grid-cols-2 gap-2">
      <Dropdown
        id={id}
        options={monthOptions}
        value={month}
        onChange={(m) => emit(m, year)}
        placeholder={t('build.monthYear.month')}
        disabled={disabled}
        searchable={false}
      />
      <Dropdown
        options={yearOptions}
        value={year}
        onChange={(y) => emit(month, y)}
        placeholder={t('build.monthYear.year')}
        disabled={disabled}
        searchable={false}
        listClassName="max-h-40"
      />
    </div>
  );
}
