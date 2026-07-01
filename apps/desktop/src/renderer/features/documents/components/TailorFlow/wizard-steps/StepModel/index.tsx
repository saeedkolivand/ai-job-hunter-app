import { Search } from 'lucide-react';
import { Controller, useFormContext } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import { ModelSelector } from '@/components/ui/ModelSelector';

import type { TailorWizardState } from '../../lib/tailor-state';

interface StepModelProps {
  canUse: boolean;
  /** Why AI is unavailable, if it is — drives the inline disabled hint. */
  reason?: string;
}

/** Final step: the global AI model + the opt-in company-research toggle. */
export function StepModel({ canUse, reason }: StepModelProps) {
  const { t } = useTranslation();
  const { control } = useFormContext<TailorWizardState>();

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.apply.wizard.model.title')}
        </p>
        <p className="mt-0.5 text-xs text-foreground/35">
          {t('autopilot.apply.wizard.model.subtitle')}
        </p>
      </div>

      <ModelSelector />

      {!canUse && (
        <p className="text-[11px] text-amber-300/70">
          {reason === 'addApiKey'
            ? t('autopilot.apply.addApiKey')
            : reason === 'installCli'
              ? t('autopilot.apply.installCli')
              : t('autopilot.apply.selectModel')}
        </p>
      )}

      <Controller
        control={control}
        name="researchCompany"
        render={({ field }) => (
          <Button
            variant="unstyled"
            type="button"
            role="switch"
            aria-checked={field.value}
            onClick={() => field.onChange(!field.value)}
            className={cn(
              'flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-left transition-all',
              field.value
                ? 'border-brand/35 bg-brand/8'
                : 'border-[var(--border-clear)] bg-transparent hover:bg-muted'
            )}
          >
            <span className="flex min-w-0 items-start gap-2">
              <Search size={13} className="mt-0.5 shrink-0 text-brand-soft" />
              <span className="min-w-0">
                <span
                  className={cn(
                    'block text-[11px] font-medium',
                    field.value ? 'text-foreground/90' : 'text-foreground/55'
                  )}
                >
                  {t('autopilot.apply.research.label')}
                </span>
                <span className="mt-0.5 block text-[10px] text-foreground/35">
                  {t('autopilot.apply.research.hint')}
                </span>
              </span>
            </span>
            <span
              className={cn(
                'relative ml-3 h-4 w-7 shrink-0 rounded-full transition-colors',
                field.value ? 'bg-brand' : 'bg-muted'
              )}
            >
              <span
                className={cn(
                  'absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-transform',
                  field.value ? 'translate-x-3.5' : 'translate-x-0.5'
                )}
              />
            </span>
          </Button>
        )}
      />
    </div>
  );
}
