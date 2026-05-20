import { useTranslation } from '@/lib/i18n';
import { cn } from '@/lib/cn';
import { BOARD_IDS } from '@ajh/shared';
import { WizardField } from './WizardField';
import { PrefilledBadge } from './PrefilledBadge';
import type { WizardState, SetFn, Prefilled } from '@/routes/autopilot';

const inputCls =
  'w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-xs text-foreground/80 placeholder:text-foreground/25 outline-none focus:border-brand/40 transition-colors';

interface StepTargetProps {
  form: WizardState;
  set: SetFn;
  prefilled: Prefilled;
}

export function StepTarget({ form, set, prefilled }: StepTargetProps) {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.wizard.target.title')}
        </p>
        <p className="text-xs text-foreground/35 mt-0.5">{t('autopilot.wizard.target.subtitle')}</p>
      </div>

      <WizardField label={t('autopilot.wizard.target.name')}>
        <input
          className={inputCls}
          placeholder={t('autopilot.wizard.target.namePlaceholder')}
          value={form.name}
          onChange={(e) => set('name', e.target.value)}
        />
      </WizardField>

      <WizardField label={t('autopilot.wizard.target.board')}>
        <div className="grid grid-cols-4 gap-1.5 max-h-28 overflow-y-auto pr-1">
          {BOARD_IDS.map((b) => (
            <button
              key={b}
              onClick={() => set('board', b)}
              className={cn(
                'rounded-lg border px-2 py-1.5 text-[10px] font-medium capitalize transition-all',
                form.board === b
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-white/[0.06] text-foreground/40 hover:border-white/10 hover:text-foreground/65'
              )}
            >
              {b}
            </button>
          ))}
        </div>
      </WizardField>

      <div className="grid grid-cols-2 gap-3">
        <WizardField label={t('autopilot.wizard.target.query')}>
          <input
            className={inputCls}
            placeholder={t('autopilot.wizard.target.queryPlaceholder')}
            value={form.query}
            onChange={(e) => set('query', e.target.value)}
          />
        </WizardField>
        <WizardField
          label={t('autopilot.wizard.target.location')}
          hint={t('autopilot.wizard.target.locationOptional')}
        >
          <div className="space-y-1.5">
            <input
              className={inputCls}
              placeholder={t('autopilot.wizard.target.locationPlaceholder')}
              value={form.location}
              onChange={(e) => set('location', e.target.value)}
            />
            {prefilled.location && (
              <PrefilledBadge field={t('autopilot.wizard.target.fromLocationSettings')} />
            )}
          </div>
        </WizardField>
      </div>

      <WizardField label={t('autopilot.wizard.target.workType')}>
        <div className="grid grid-cols-4 gap-1.5">
          {(['any', 'remote', 'hybrid', 'on-site'] as const).map((opt) => (
            <button
              key={opt}
              onClick={() => set('workType', opt)}
              className={cn(
                'rounded-lg border px-2 py-1.5 text-[10px] font-medium capitalize transition-all',
                form.workType === opt
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-white/[0.06] text-foreground/40 hover:border-white/10 hover:text-foreground/65'
              )}
            >
              {opt}
            </button>
          ))}
        </div>
      </WizardField>

      <div className="grid grid-cols-2 gap-3">
        <WizardField label={t('autopilot.wizard.target.pages')}>
          <input
            type="number"
            min={1}
            max={10}
            className={inputCls}
            value={form.pages}
            onChange={(e) => set('pages', Number(e.target.value))}
          />
        </WizardField>
        <WizardField label={t('autopilot.wizard.target.postedWithin')}>
          <select
            className={inputCls}
            value={form.dateFilter}
            onChange={(e) => set('dateFilter', e.target.value)}
          >
            <option value="">{t('autopilot.wizard.target.anyTime')}</option>
            <option value="24h">{t('autopilot.wizard.target.last24h')}</option>
            <option value="week">{t('autopilot.wizard.target.lastWeek')}</option>
            <option value="month">{t('autopilot.wizard.target.lastMonth')}</option>
          </select>
        </WizardField>
      </div>
    </div>
  );
}
