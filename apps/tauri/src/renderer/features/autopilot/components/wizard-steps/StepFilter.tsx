import { Input, TextArea } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import type { Prefilled, SetFn, WizardState } from '@/routes/autopilot';

import { PrefilledBadge } from './PrefilledBadge';
import { WizardField } from './WizardField';

const inputCls =
  'w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-xs text-foreground/80 placeholder:text-foreground/25 outline-none focus:border-brand/40 transition-colors';

interface StepFilterProps {
  form: WizardState;
  set: SetFn;
  prefilled: Prefilled;
}

export function StepFilter({ form, set, prefilled }: StepFilterProps) {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.wizard.filter.title')}
        </p>
        <p className="text-xs text-foreground/35 mt-0.5">{t('autopilot.wizard.filter.subtitle')}</p>
      </div>

      <WizardField
        label={`${t('autopilot.wizard.filter.minMatchScore', { score: form.minMatchScore })}`}
        hint={t('autopilot.wizard.filter.minMatchScoreHint')}
      >
        <div className="flex items-center gap-3">
          <input
            type="range"
            min={0}
            max={100}
            step={5}
            value={form.minMatchScore}
            onChange={(e) => set('minMatchScore', Number(e.target.value))}
            className="flex-1 accent-[var(--color-brand)]"
          />
          <span className="text-xs font-mono text-foreground/50 w-8 text-right">
            {form.minMatchScore}%
          </span>
        </div>
        <p className="text-[10px] text-foreground/30">
          {t('autopilot.wizard.filter.minMatchScoreDesc')}
        </p>
      </WizardField>

      <WizardField
        label={t('autopilot.wizard.filter.resume')}
        hint={t('autopilot.wizard.filter.resumeOptional')}
      >
        <TextArea
          value={form.resumeText}
          onChange={(e) => set('resumeText', e.target.value)}
          placeholder={t('autopilot.wizard.filter.resumePlaceholder')}
          className="w-full bg-white/[0.02] text-[11px] text-foreground/70 placeholder:text-foreground/20 font-mono leading-relaxed"
          rows={4}
          spellCheck={false}
        />
      </WizardField>

      <div className="grid grid-cols-2 gap-3">
        <WizardField
          label={t('autopilot.wizard.filter.mustInclude')}
          hint={t('autopilot.wizard.filter.commaSeparated')}
        >
          <div className="space-y-1.5">
            <Input
              className={inputCls}
              placeholder={t('autopilot.wizard.filter.keywordsPlaceholder')}
              value={form.keywords}
              onChange={(e) => set('keywords', e.target.value)}
            />
            {prefilled.keywords && (
              <PrefilledBadge field={t('autopilot.wizard.filter.fromTechStackSettings')} />
            )}
          </div>
        </WizardField>
        <WizardField
          label={t('autopilot.wizard.filter.excludeKeywords')}
          hint={t('autopilot.wizard.filter.commaSeparated')}
        >
          <Input
            className={inputCls}
            placeholder={t('autopilot.wizard.filter.excludePlaceholder')}
            value={form.excludeKeywords}
            onChange={(e) => set('excludeKeywords', e.target.value)}
          />
        </WizardField>
      </div>
    </div>
  );
}
