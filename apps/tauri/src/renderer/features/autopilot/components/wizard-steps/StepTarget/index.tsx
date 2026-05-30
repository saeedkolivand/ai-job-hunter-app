import { BOARD_IDS } from '@ajh/shared';
import { Button, cn, Input, LocationInput, SelectDropdown } from '@ajh/ui';

import type { Prefilled, SetFn, WizardState } from '@/features/autopilot/types';
import { useTranslation } from '@/lib/i18n';
import { useAppClient } from '@/providers/AppClientProvider';

import { ComingSoonBadge } from '../ComingSoonBadge';
import { PrefilledBadge } from '../PrefilledBadge';
import { WizardField } from '../WizardField';

const inputCls =
  'w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-xs text-foreground/80 placeholder:text-foreground/25 outline-none focus:border-brand/40 transition-colors';

interface StepTargetProps {
  form: WizardState;
  set: SetFn;
  prefilled: Prefilled;
}

export function StepTarget({ form, set, prefilled }: StepTargetProps) {
  const { t } = useTranslation();
  const api = useAppClient();
  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.wizard.target.title')}
        </p>
        <p className="text-xs text-foreground/35 mt-0.5">{t('autopilot.wizard.target.subtitle')}</p>
      </div>

      <WizardField label={t('autopilot.wizard.target.name')}>
        <Input
          className={inputCls}
          placeholder={t('autopilot.wizard.target.namePlaceholder')}
          value={form.name}
          onChange={(e) => set('name', e.target.value)}
        />
      </WizardField>

      <WizardField label={t('autopilot.wizard.target.board')}>
        <div className="grid grid-cols-4 gap-1.5 max-h-28 overflow-y-auto pr-1">
          {BOARD_IDS.map((b) => (
            <Button
              key={b}
              onClick={() => set('board', b)}
              className={cn(
                'rounded-lg border px-2 py-1.5 text-[10px] font-medium capitalize transition-all h-auto',
                form.board === b
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-white/[0.06] text-foreground/40 hover:border-white/10 hover:text-foreground/65'
              )}
            >
              {b}
            </Button>
          ))}
        </div>
      </WizardField>

      <div className="grid grid-cols-2 gap-3">
        <WizardField label={t('autopilot.wizard.target.query')}>
          <Input
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
            <LocationInput
              value={form.location}
              onChange={(value) => set('location', value)}
              placeholder={t('autopilot.wizard.target.locationPlaceholder')}
              onFetchSuggestions={(q) => api.geocode.suggest(q)}
            />
            {prefilled.location && (
              <PrefilledBadge field={t('autopilot.wizard.target.fromLocationSettings')} />
            )}
          </div>
        </WizardField>
      </div>

      <WizardField label={t('autopilot.wizard.target.workType')} badge={<ComingSoonBadge />}>
        <div className="grid grid-cols-4 gap-1.5">
          {(['any', 'remote', 'hybrid', 'on-site'] as const).map((opt) => (
            <Button
              key={opt}
              disabled
              className={cn(
                'rounded-lg border px-2 py-1.5 text-[10px] font-medium capitalize transition-all h-auto',
                form.workType === opt
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-white/[0.06] text-foreground/40'
              )}
            >
              {opt}
            </Button>
          ))}
        </div>
      </WizardField>

      <div className="grid grid-cols-2 gap-3">
        <WizardField label={t('autopilot.wizard.target.pages')}>
          <Input
            type="number"
            min={1}
            max={10}
            className={inputCls}
            value={form.pages}
            onChange={(e) => set('pages', Number(e.target.value))}
          />
        </WizardField>
        <WizardField label={t('autopilot.wizard.target.postedWithin')}>
          <SelectDropdown
            options={[
              { value: '', label: t('autopilot.wizard.target.anyTime') },
              { value: '24h', label: t('autopilot.wizard.target.last24h') },
              { value: 'week', label: t('autopilot.wizard.target.lastWeek') },
              { value: 'month', label: t('autopilot.wizard.target.lastMonth') },
            ]}
            value={form.dateFilter}
            onChange={(value) => set('dateFilter', value)}
            placeholder={t('autopilot.wizard.target.anyTime')}
          />
        </WizardField>
      </div>
    </div>
  );
}
