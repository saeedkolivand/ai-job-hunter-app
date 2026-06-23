import { Controller, useFormContext } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Button, cn, Input } from '@ajh/ui';

import { ResumeInputCard } from '@/components/resume/ResumeInputCard';
import type { WizardState } from '@/features/autopilot/types';
import { MATCH_LEVELS, scoreToLevel } from '@/lib/match-level';

import { WizardField } from '../WizardField';

const fieldCls = 'h-9 w-full text-xs shadow-none';

export function StepFilter() {
  const { t } = useTranslation();
  const { control } = useFormContext<WizardState>();

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.wizard.filter.title')}
        </p>
        <p className="text-xs text-foreground/35 mt-0.5">{t('autopilot.wizard.filter.subtitle')}</p>
      </div>

      <Controller
        control={control}
        name="minMatchScore"
        render={({ field }) => {
          const active = field.value <= 0 ? 'any' : scoreToLevel(field.value);
          const levels = [{ id: 'any' as const, value: 0 }, ...MATCH_LEVELS];
          return (
            <WizardField
              label={t('autopilot.wizard.filter.matchScore')}
              hint={t('autopilot.wizard.filter.matchScoreHint')}
            >
              <div className="grid grid-cols-2 gap-1.5 @xs:grid-cols-4">
                {levels.map(({ id, value }) => (
                  <Button
                    key={id}
                    aria-pressed={active === id}
                    onClick={() => field.onChange(value)}
                    className={cn(
                      'flex flex-col items-center gap-0.5 rounded-lg border px-2 py-2 transition-all h-auto',
                      active === id
                        ? 'border-brand/40 bg-brand/10 text-brand-soft'
                        : 'border-[var(--border-clear)] bg-card text-foreground/45 hover:bg-muted hover:text-foreground/70'
                    )}
                  >
                    <span className="text-xs font-semibold capitalize">
                      {t(`autopilot.wizard.filter.matchLevel.${id}`)}
                    </span>
                    <span className="text-[9px] text-foreground/35">
                      {t(`autopilot.wizard.filter.matchLevel.${id}Desc`)}
                    </span>
                  </Button>
                ))}
              </div>
              <p className="text-[10px] text-foreground/30">
                {t('autopilot.wizard.filter.minMatchScoreDesc')}
              </p>
            </WizardField>
          );
        }}
      />

      <Controller
        control={control}
        name="resumeText"
        render={({ field }) => (
          <ResumeInputCard
            value={field.value}
            onChange={field.onChange}
            placeholder={t('autopilot.wizard.filter.resumePlaceholder')}
          />
        )}
      />

      <div className="grid grid-cols-1 gap-3 @xs:grid-cols-2">
        <Controller
          control={control}
          name="keywords"
          render={({ field }) => (
            <WizardField
              label={t('autopilot.wizard.filter.mustInclude')}
              hint={t('autopilot.wizard.filter.commaSeparated')}
              htmlFor="autopilot-keywords"
            >
              <div className="space-y-1.5">
                <Input
                  id="autopilot-keywords"
                  variant="default"
                  className={fieldCls}
                  placeholder={t('autopilot.wizard.filter.keywordsPlaceholder')}
                  value={field.value}
                  onChange={field.onChange}
                  onBlur={field.onBlur}
                />
              </div>
            </WizardField>
          )}
        />
        <Controller
          control={control}
          name="excludeKeywords"
          render={({ field }) => (
            <WizardField
              label={t('autopilot.wizard.filter.excludeKeywords')}
              hint={t('autopilot.wizard.filter.commaSeparated')}
              htmlFor="autopilot-exclude-keywords"
            >
              <Input
                id="autopilot-exclude-keywords"
                variant="default"
                className={fieldCls}
                placeholder={t('autopilot.wizard.filter.excludePlaceholder')}
                value={field.value}
                onChange={field.onChange}
                onBlur={field.onBlur}
              />
            </WizardField>
          )}
        />
      </div>
    </div>
  );
}
