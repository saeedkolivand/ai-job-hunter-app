import { Check, Clock } from 'lucide-react';
import { useFormContext, useWatch } from 'react-hook-form';

import type { AutopilotSchedule } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, cn, Dropdown } from '@ajh/ui';

import type { WizardState } from '@/features/autopilot/types';
import { scoreToLevel } from '@/lib/match-level';

import { WizardField } from '../WizardField';

/** Zero-padded two-digit string (e.g. 9 → "09"). */
const pad2 = (n: number) => String(n).padStart(2, '0');

/** "HH:MM" for an hour/minute pair. */
const formatTime = (hour: number, minute: number) => `${pad2(hour)}:${pad2(minute)}`;

const HOUR_OPTIONS = Array.from({ length: 24 }, (_, h) => ({
  value: String(h),
  label: pad2(h),
}));

// Minute granularity for the dropdown — every 5 minutes (00, 05, … 55).
const MINUTE_OPTIONS = Array.from({ length: 12 }, (_, i) => ({
  value: String(i * 5),
  label: pad2(i * 5),
}));

export function StepSchedule() {
  const { t } = useTranslation();
  const { control, setValue } = useFormContext<WizardState>();
  // The schedule step is a derived/multi-control view: watch the fields it reads
  // and write through setValue (the value-array controls don't map cleanly to a
  // single Controller). RHF stays the single source of truth.
  const name = useWatch({ control, name: 'name' });
  const board = useWatch({ control, name: 'board' });
  const query = useWatch({ control, name: 'query' });
  const minMatchScore = useWatch({ control, name: 'minMatchScore' });
  const schedule = useWatch({ control, name: 'schedule' });
  const scheduleHour = useWatch({ control, name: 'scheduleHour' });
  const scheduleMinute = useWatch({ control, name: 'scheduleMinute' });

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

  const showHourMinute = schedule === 'daily' || schedule === 'twice_daily';
  const showMinuteOnly = schedule === 'hourly';
  const secondTime = formatTime((scheduleHour + 12) % 24, scheduleMinute);

  // Schedule label + time, formatted for the summary line.
  const scheduleSummary = (() => {
    switch (schedule) {
      case 'daily':
        return `${t('autopilot.wizard.schedule.daily')} · ${formatTime(scheduleHour, scheduleMinute)}`;
      case 'twice_daily':
        return `${t('autopilot.wizard.schedule.twiceDaily')} · ${formatTime(scheduleHour, scheduleMinute)} & ${secondTime}`;
      case 'hourly':
        return `${t('autopilot.wizard.schedule.hourly')} · :${pad2(scheduleMinute)}`;
      default:
        return t('autopilot.wizard.schedule.manual');
    }
  })();

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
            onClick={() => setValue('schedule', id, { shouldDirty: true, shouldValidate: true })}
            className={cn(
              'w-full flex items-center gap-3 rounded-xl border px-4 py-3 text-left transition-all h-auto',
              schedule === id
                ? 'border-brand/35 bg-brand/10'
                : 'border-white/[0.05] hover:border-white/[0.08]'
            )}
          >
            <Clock
              size={13}
              className={schedule === id ? 'text-brand-soft' : 'text-foreground/30'}
            />
            <div className="flex-1">
              <div className="text-xs font-semibold text-foreground/75">{label}</div>
              <div className="text-[10px] text-foreground/40">{desc}</div>
            </div>
            {schedule === id && <Check size={12} className="text-brand-soft" />}
          </Button>
        ))}
      </div>

      {/* Time control — gated by the selected schedule. */}
      {showHourMinute && (
        <WizardField label={t('autopilot.wizard.schedule.runAt')}>
          <div className="flex items-end gap-2">
            <div className="flex-1 space-y-1">
              <label htmlFor="schedule-hour" className="text-[10px] text-foreground/35">
                {t('autopilot.wizard.schedule.hourLabel')}
              </label>
              <Dropdown
                id="schedule-hour"
                options={HOUR_OPTIONS}
                value={String(scheduleHour)}
                onChange={(v) => setValue('scheduleHour', Number(v), { shouldDirty: true })}
              />
            </div>
            <span className="pb-2 text-sm font-medium text-foreground/40">:</span>
            <div className="flex-1 space-y-1">
              <label htmlFor="schedule-minute" className="text-[10px] text-foreground/35">
                {t('autopilot.wizard.schedule.minuteLabel')}
              </label>
              <Dropdown
                id="schedule-minute"
                options={MINUTE_OPTIONS}
                value={String(scheduleMinute)}
                onChange={(v) => setValue('scheduleMinute', Number(v), { shouldDirty: true })}
              />
            </div>
          </div>
          {schedule === 'twice_daily' && (
            <p className="text-[10px] text-foreground/40">
              {t('autopilot.wizard.schedule.alsoRunsAt', {
                first: formatTime(scheduleHour, scheduleMinute),
                second: secondTime,
              })}
            </p>
          )}
        </WizardField>
      )}

      {showMinuteOnly && (
        <WizardField
          label={t('autopilot.wizard.schedule.minutesPastHour')}
          htmlFor="schedule-minutes-past-hour"
        >
          <Dropdown
            id="schedule-minutes-past-hour"
            options={MINUTE_OPTIONS}
            value={String(scheduleMinute)}
            onChange={(v) => setValue('scheduleMinute', Number(v), { shouldDirty: true })}
          />
        </WizardField>
      )}

      {/* Summary */}
      <div className="rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3 space-y-1.5">
        <div className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('autopilot.wizard.schedule.summary')}
        </div>
        {[
          [t('autopilot.wizard.schedule.summaryName'), name || '—'],
          [t('autopilot.wizard.schedule.summaryBoard'), board],
          [t('autopilot.wizard.schedule.summaryQuery'), query || '—'],
          [t('autopilot.wizard.schedule.summarySchedule'), scheduleSummary],
          [
            t('autopilot.wizard.schedule.summaryMinScore'),
            t(`autopilot.wizard.filter.matchLevel.${scoreToLevel(minMatchScore)}`),
          ],
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
