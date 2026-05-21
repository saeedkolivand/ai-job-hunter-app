import { DollarSign } from 'lucide-react';
import { useState } from 'react';

import { Button, GlassCard } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import type { SalaryExpectation } from '@/store/preferences-schema';
import { usePreferencesStore, useSalary } from '@/store/preferences-store';

const CURRENCIES = ['USD', 'EUR', 'GBP', 'CAD', 'AUD', 'JPY'] as const;
const PERIODS = ['hourly', 'monthly', 'yearly'] as const;

const SALARY_RANGES = {
  hourly: { min: 10, max: 200, step: 5 },
  monthly: { min: 1000, max: 50000, step: 500 },
  yearly: { min: 20000, max: 500000, step: 5000 },
};

const formatSalary = (value: number, period: string) => {
  if (period === 'yearly') {
    return `$${(value / 1000).toFixed(0)}k`;
  }
  return `$${value.toLocaleString()}`;
};

export function SalaryPreferences() {
  const { t } = useTranslation();
  const salary = useSalary();
  const setSalary = usePreferencesStore((state) => state.setSalary);

  const [minValue, setMinValue] = useState(salary?.min || 50000);
  const [maxValue, setMaxValue] = useState(salary?.max || 150000);
  const [currency, setCurrency] = useState(salary?.currency || 'USD');
  const [period, setPeriod] = useState<SalaryExpectation['period']>(salary?.period || 'yearly');

  const handleMinChange = (value: number) => {
    const newMin = Math.min(value, maxValue - 10000);
    setMinValue(newMin);
    setSalary({ min: newMin, max: maxValue, currency, period });
  };

  const handleMaxChange = (value: number) => {
    const newMax = Math.max(value, minValue + 10000);
    setMaxValue(newMax);
    setSalary({ min: minValue, max: newMax, currency, period });
  };

  const handleCurrencyChange = (newCurrency: string) => {
    setCurrency(newCurrency);
    setSalary({ min: minValue, max: maxValue, currency: newCurrency, period });
  };

  const handlePeriodChange = (newPeriod: SalaryExpectation['period']) => {
    setPeriod(newPeriod);
    const range = SALARY_RANGES[newPeriod];
    setMinValue(range.min);
    setMaxValue(range.max * 0.5);
    setSalary({ min: range.min, max: range.max * 0.5, currency, period: newPeriod });
  };

  const range = SALARY_RANGES[period];

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          {t('settings.salary.title')}
        </div>
      </div>

      <p className="mb-4 text-sm text-foreground/55">{t('settings.salary.description')}</p>

      {/* Currency & Period Selection */}
      <div className="mb-6 grid grid-cols-2 gap-3">
        <div>
          <div className="mb-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            {t('settings.salary.currency')}
          </div>
          <div className="flex flex-wrap gap-2">
            {CURRENCIES.map((curr) => (
              <Button
                key={curr}
                onClick={() => handleCurrencyChange(curr)}
                className={cn(
                  'rounded-lg px-3 py-1.5 text-sm font-medium transition-colors h-auto',
                  currency === curr
                    ? 'bg-brand-soft/20 text-brand-soft'
                    : 'bg-white/5 text-foreground/60 hover:bg-white/10 hover:text-foreground'
                )}
              >
                {curr}
              </Button>
            ))}
          </div>
        </div>

        <div>
          <div className="mb-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            {t('settings.salary.period')}
          </div>
          <div className="flex flex-wrap gap-2">
            {PERIODS.map((per) => (
              <Button
                key={per}
                onClick={() => handlePeriodChange(per)}
                className={cn(
                  'rounded-lg px-3 py-1.5 text-sm font-medium transition-colors capitalize h-auto',
                  period === per
                    ? 'bg-brand-soft/20 text-brand-soft'
                    : 'bg-white/5 text-foreground/60 hover:bg-white/10 hover:text-foreground'
                )}
              >
                {per}
              </Button>
            ))}
          </div>
        </div>
      </div>

      {/* Salary Range Slider */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            {t('settings.salary.range')}
          </div>
          <div className="flex items-center gap-2 text-sm">
            <DollarSign size={16} className="text-foreground/40" />
            <span className="text-foreground">
              {formatSalary(minValue, period)} - {formatSalary(maxValue, period)}
            </span>
          </div>
        </div>

        <div className="space-y-3">
          {/* Min Slider */}
          <div>
            <div className="mb-2 flex justify-between text-sm">
              <span className="text-foreground/60">{t('settings.salary.minimum')}</span>
              <span className="text-foreground">{formatSalary(minValue, period)}</span>
            </div>
            <input
              type="range"
              min={range.min}
              max={range.max}
              step={range.step}
              value={minValue}
              onChange={(e) => handleMinChange(Number(e.target.value))}
              className="w-full h-2 appearance-none rounded-lg bg-white/5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-brand-soft [&::-webkit-slider-thumb]:cursor-pointer [&::-webkit-slider-thumb]:transition-transform [&::-webkit-slider-thumb]:hover:scale-110"
            />
          </div>

          {/* Max Slider */}
          <div>
            <div className="mb-2 flex justify-between text-sm">
              <span className="text-foreground/60">{t('settings.salary.maximum')}</span>
              <span className="text-foreground">{formatSalary(maxValue, period)}</span>
            </div>
            <input
              type="range"
              min={range.min}
              max={range.max}
              step={range.step}
              value={maxValue}
              onChange={(e) => handleMaxChange(Number(e.target.value))}
              className="w-full h-2 appearance-none rounded-lg bg-white/5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-brand-soft [&::-webkit-slider-thumb]:cursor-pointer [&::-webkit-slider-thumb]:transition-transform [&::-webkit-slider-thumb]:hover:scale-110"
            />
          </div>
        </div>
      </div>
    </GlassCard>
  );
}
