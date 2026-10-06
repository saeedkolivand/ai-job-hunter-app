import { Controller, useFormContext } from 'react-hook-form';

import { WORK_TYPE_OPTIONS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import type { WizardState } from '@/features/autopilot/types';

import { WizardField } from '../WizardField';

/** Work-type multi-select (empty = any). */
export function WorkTypePicker() {
  const { t } = useTranslation();
  const { control } = useFormContext<WizardState>();
  return (
    <Controller
      control={control}
      name="workTypes"
      render={({ field }) => {
        const sel = new Set(field.value);
        const toggle = (opt: (typeof WORK_TYPE_OPTIONS)[number]) => {
          field.onChange(
            sel.has(opt) ? field.value.filter((w) => w !== opt) : [...field.value, opt]
          );
        };
        return (
          <WizardField
            label={t('autopilot.wizard.target.workType')}
            // Empty set silently means "any" — three neutral, identically
            // unselected buttons read as broken/unset otherwise. Same
            // "Any time"-style visible microcopy idiom as the Posted
            // Dropdown's own empty state.
            hint={field.value.length === 0 ? t('jobs.workType.any') : undefined}
          >
            {/* Multi-select set, not a Dropdown — a Dropdown can't express a
                set. Empty = any, all three = all. Mirrors the board picker
                and ScrapeForm's manual-search control — including its
                visual language (same selected/unselected classes) and its
                flex-wrap (not a fixed grid column, which can clip "Vor
                Ort"). Plain tab stops, not roving tabindex: that pattern
                earns its keep on the ~26-item board picker, but a
                3-item set has no efficiency win from it and it breaks the
                standard "Tab moves to the next toggle" expectation. */}
            <div
              role="group"
              aria-label={t('autopilot.wizard.target.workType')}
              className="flex flex-wrap gap-1.5"
            >
              {WORK_TYPE_OPTIONS.map((opt) => {
                const active = sel.has(opt);
                return (
                  <Button
                    key={opt}
                    aria-pressed={active}
                    variant="ghost"
                    onClick={() => toggle(opt)}
                    className={cn(
                      'rounded-lg px-2.5 py-1 text-[11px] transition-all',
                      active
                        ? 'bg-brand/20 text-brand-soft ring-1 ring-brand/40'
                        : 'bg-card border border-[var(--border-clear)] text-foreground/50 hover:bg-muted hover:text-foreground/80'
                    )}
                  >
                    {t(`jobs.workType.${opt}`)}
                  </Button>
                );
              })}
            </div>
          </WizardField>
        );
      }}
    />
  );
}
