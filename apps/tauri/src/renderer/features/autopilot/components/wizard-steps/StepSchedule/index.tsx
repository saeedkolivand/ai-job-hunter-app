import { Check, Clock } from 'lucide-react';

import type { AutopilotSchedule } from '@ajh/shared';
import { Button, cn } from '@ajh/ui';

import type { SetFn, WizardState } from '@/features/autopilot/types';
import { useTranslation } from '@/lib/i18n';

interface StepScheduleProps {
  form: WizardState;
  set: SetFn;
}

export function StepSchedule({ form, set }: StepScheduleProps) {
  const { t } = useTranslation();
  const scheduleOptions = [
    {
      id: 'manual' as AutopilotSchedule,
      label: t('autopilot.wizard.schedule.manual'),
      desc: t('autopilot.wizard.schedule.manualDesc'),
    },
    {
      id: 'hourly' as AutopilotSchedule,
      label: t('autopilot.wizard.schedule.hourly'),
      desc: t('autopilot.wizard.schedule.hourlyDesc'),
    },
    {
      id: 'daily' as AutopilotSchedule,
      label: t('autopilot.wizard.schedule.daily'),
      desc: t('autopilot.wizard.schedule.dailyDesc'),
    },
    {
      id: 'twice_daily' as AutopilotSchedule,
      label: t('autopilot.wizard.schedule.twiceDaily'),
      desc: t('autopilot.wizard.schedule.twiceDailyDesc'),
    },
  ];

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.wizard.schedule.title')}
        </p>
        <p className="text-xs text-foreground/35 mt-0.5">
          {t('autopilot.wizard.schedule.subtitle')}
        </p>
      </div>

      <div className="space-y-2">
        {scheduleOptions.map(({ id, label, desc }) => (
          <Button
            key={id}
            onClick={() => set('schedule', id)}
            className={cn(
              'w-full flex items-center gap-3 rounded-xl border px-4 py-3 text-left transition-all h-auto',
              form.schedule === id
                ? 'border-brand/35 bg-brand/10'
                : 'border-white/[0.05] hover:border-white/[0.08]'
            )}
          >
            <Clock
              size={13}
              className={form.schedule === id ? 'text-brand-soft' : 'text-foreground/30'}
            />
            <div className="flex-1">
              <div className="text-xs font-semibold text-foreground/75">{label}</div>
              <div className="text-[10px] text-foreground/40">{desc}</div>
            </div>
            {form.schedule === id && <Check size={12} className="text-brand-soft" />}
          </Button>
        ))}
      </div>

      {/* Summary */}
      <div className="rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3 space-y-1.5">
        <div className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('autopilot.wizard.schedule.summary')}
        </div>
        {[
          [t('autopilot.wizard.schedule.summaryName'), form.name || '—'],
          [t('autopilot.wizard.schedule.summaryBoard'), form.board],
          [t('autopilot.wizard.schedule.summaryQuery'), form.query || '—'],
          [t('autopilot.wizard.schedule.summarySchedule'), form.schedule.replace('_', ' ')],
          [t('autopilot.wizard.schedule.summaryMinScore'), `${form.minMatchScore}%`],
        ].map(([k, v]) => (
          <div key={k} className="flex items-center justify-between">
            <span className="text-[10px] text-foreground/35">{k}</span>
            <span className="text-[10px] font-medium text-foreground/65 capitalize">{v}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
