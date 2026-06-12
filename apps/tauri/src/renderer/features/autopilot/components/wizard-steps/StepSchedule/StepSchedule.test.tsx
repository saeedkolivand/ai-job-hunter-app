import { FormProvider, useForm, useFormContext } from 'react-hook-form';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { WizardState } from '@/features/autopilot/types';

import { StepSchedule } from './index';

// ── Module stubs ──────────────────────────────────────────────────────────────

// Return every translation key verbatim — no i18next setup needed in jsdom.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, string>) => {
      if (!params) return key;
      // Substitute {{placeholder}} values for tests that inspect interpolated strings.
      return Object.entries(params).reduce(
        (acc, [k, v]) => acc.replace(new RegExp(`\\{\\{${k}\\}\\}`, 'g'), String(v)),
        key
      );
    },
  }),
}));

// ── Fixture helpers ───────────────────────────────────────────────────────────

function makeForm(overrides: Partial<WizardState> = {}): WizardState {
  return {
    name: 'Test run',
    board: 'linkedin',
    query: 'react developer',
    location: 'Berlin',
    workType: 'remote',
    amount: 50,
    dateFilter: '24h',
    minMatchScore: 50,
    keywords: '',
    excludeKeywords: '',
    resumeText: '',
    schedule: 'daily',
    scheduleHour: 9,
    scheduleMinute: 0,
    ...overrides,
  };
}

/** Surfaces the live RHF schedule fields as JSON so tests can assert value + type. */
function Probe() {
  const { watch } = useFormContext<WizardState>();
  const v = watch();
  return (
    <output data-testid="probe">
      {JSON.stringify({
        schedule: v.schedule,
        scheduleHour: v.scheduleHour,
        scheduleMinute: v.scheduleMinute,
      })}
    </output>
  );
}

/** Render StepSchedule inside a real RHF form seeded with `overrides`. */
function renderStep(overrides: Partial<WizardState> = {}) {
  function Host() {
    const methods = useForm<WizardState>({ defaultValues: makeForm(overrides) });
    return (
      <FormProvider {...methods}>
        <StepSchedule />
        <Probe />
      </FormProvider>
    );
  }
  return render(<Host />);
}

function readProbe(): Pick<WizardState, 'schedule' | 'scheduleHour' | 'scheduleMinute'> {
  return JSON.parse(screen.getByTestId('probe').textContent ?? '{}');
}

/** Open the Dropdown identified by its trigger button id attribute and pick an option. */
async function pickOptionById(
  user: ReturnType<typeof userEvent.setup>,
  triggerId: string,
  optionLabel: string
) {
  // Dropdown forwards an `id` onto its trigger button.
  const trigger = document.getElementById(triggerId);
  if (!trigger) throw new Error(`No element with id="${triggerId}"`);
  await user.click(trigger);
  expect(screen.getByRole('listbox')).toBeInTheDocument();
  await user.click(screen.getByRole('option', { name: optionLabel }));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('StepSchedule', () => {
  // ── Gating: control visibility per schedule ─────────────────────────────────

  describe('gating — daily', () => {
    it('renders both hour and minute controls', () => {
      renderStep({ schedule: 'daily' });
      expect(document.getElementById('schedule-hour')).toBeInTheDocument();
      expect(document.getElementById('schedule-minute')).toBeInTheDocument();
    });

    it('does not render the minutes-past-hour control', () => {
      renderStep({ schedule: 'daily' });
      expect(document.getElementById('schedule-minutes-past-hour')).not.toBeInTheDocument();
    });
  });

  describe('gating — twice_daily', () => {
    it('renders both hour and minute controls', () => {
      renderStep({ schedule: 'twice_daily' });
      expect(document.getElementById('schedule-hour')).toBeInTheDocument();
      expect(document.getElementById('schedule-minute')).toBeInTheDocument();
    });

    it('shows a "+12h" hint containing the second run time (hour 9 → 21:00)', () => {
      renderStep({ schedule: 'twice_daily', scheduleHour: 9, scheduleMinute: 0 });
      // The hint text contains "21:00" — the first run at 09:00 plus 12h.
      expect(screen.getByText(/21:00/)).toBeInTheDocument();
    });

    it('computes the second run time correctly for hour 13 → 01:00 (wraps at midnight)', () => {
      renderStep({ schedule: 'twice_daily', scheduleHour: 13, scheduleMinute: 15 });
      // (13 + 12) % 24 = 1 → "01:15"
      expect(screen.getByText(/01:15/)).toBeInTheDocument();
    });

    it('scheduleHour=0 boundary: "+12h" hint contains "12:00" and hour trigger shows "00" not the placeholder', async () => {
      const user = userEvent.setup();
      renderStep({ schedule: 'twice_daily', scheduleHour: 0, scheduleMinute: 0 });

      // (a) The "+12h" hint must contain "12:00" — (0+12)%24 = 12, minute 0.
      expect(screen.getByText(/12:00/)).toBeInTheDocument();

      // (b) The hour trigger's displayed label must be "00", not the placeholder "Select…".
      // Dropdown looks up `options.find(o => o.value === value)`; for scheduleHour=0
      // the value prop = String(0) = "0" matches HOUR_OPTIONS[0] → label "00". A naive
      // falsy guard on value would fall through to the placeholder instead.
      const hourTrigger = document.getElementById('schedule-hour');
      if (!hourTrigger) throw new Error('schedule-hour trigger not found');

      await user.click(hourTrigger);
      expect(screen.getByRole('listbox')).toBeInTheDocument();
      const zeroOption = screen.getByRole('option', { name: '00' });
      expect(zeroOption).toHaveAttribute('aria-selected', 'true');

      expect(hourTrigger).toHaveTextContent('00');
      expect(hourTrigger).not.toHaveTextContent('Select…');
    });
  });

  describe('gating — hourly', () => {
    it('renders the minutes-past-hour control', () => {
      renderStep({ schedule: 'hourly' });
      expect(document.getElementById('schedule-minutes-past-hour')).toBeInTheDocument();
    });

    it('does not render the hour control', () => {
      renderStep({ schedule: 'hourly' });
      expect(document.getElementById('schedule-hour')).not.toBeInTheDocument();
    });
  });

  describe('gating — manual', () => {
    it('renders no time controls at all', () => {
      renderStep({ schedule: 'manual' });
      expect(document.getElementById('schedule-hour')).not.toBeInTheDocument();
      expect(document.getElementById('schedule-minute')).not.toBeInTheDocument();
      expect(document.getElementById('schedule-minutes-past-hour')).not.toBeInTheDocument();
    });
  });

  // ── Wiring: setValue writes the correctly typed value into the form ──────────

  describe('wiring — hour control', () => {
    it('writes scheduleHour as a number when a new hour is selected', async () => {
      const user = userEvent.setup();
      renderStep({ schedule: 'daily', scheduleHour: 9 });
      await pickOptionById(user, 'schedule-hour', '08');
      const { scheduleHour } = readProbe();
      expect(scheduleHour).toBe(8);
      expect(typeof scheduleHour).toBe('number');
    });
  });

  describe('wiring — minute control (daily)', () => {
    it('writes scheduleMinute as a number when a new minute is selected', async () => {
      const user = userEvent.setup();
      renderStep({ schedule: 'daily', scheduleMinute: 0 });
      await pickOptionById(user, 'schedule-minute', '15');
      const { scheduleMinute } = readProbe();
      expect(scheduleMinute).toBe(15);
      expect(typeof scheduleMinute).toBe('number');
    });
  });

  describe('wiring — minute control (hourly)', () => {
    it('writes scheduleMinute as a number for the minutes-past-hour control', async () => {
      const user = userEvent.setup();
      renderStep({ schedule: 'hourly', scheduleMinute: 0 });
      await pickOptionById(user, 'schedule-minutes-past-hour', '30');
      const { scheduleMinute } = readProbe();
      expect(scheduleMinute).toBe(30);
      expect(typeof scheduleMinute).toBe('number');
    });
  });

  describe('wiring — schedule selector buttons', () => {
    it('sets schedule to "hourly" when the hourly button is clicked', async () => {
      const user = userEvent.setup();
      renderStep({ schedule: 'daily' });
      await user.click(
        screen.getByRole('button', { name: /autopilot\.wizard\.schedule\.hourly/i })
      );
      expect(readProbe().schedule).toBe('hourly');
    });

    it('sets schedule to "manual" when the manual button is clicked', async () => {
      const user = userEvent.setup();
      renderStep({ schedule: 'daily' });
      await user.click(
        screen.getByRole('button', { name: /autopilot\.wizard\.schedule\.manual/i })
      );
      expect(readProbe().schedule).toBe('manual');
    });
  });

  // ── Summary section ─────────────────────────────────────────────────────────

  describe('summary line', () => {
    it('daily: shows "HH:MM" formatted time (09:00 for hour 9 minute 0)', () => {
      renderStep({ schedule: 'daily', scheduleHour: 9, scheduleMinute: 0 });
      expect(screen.getByText(/09:00/)).toBeInTheDocument();
    });

    it('twice_daily: shows both times in the summary', () => {
      renderStep({ schedule: 'twice_daily', scheduleHour: 9, scheduleMinute: 0 });
      // Summary contains "09:00 & 21:00".
      expect(screen.getByText(/09:00.*21:00/s)).toBeInTheDocument();
    });

    it('hourly: shows ":MM" format (":00" for minute 0)', () => {
      renderStep({ schedule: 'hourly', scheduleMinute: 0 });
      expect(screen.getByText(/:00/)).toBeInTheDocument();
    });

    it('manual: shows the manual i18n key (no time component)', () => {
      renderStep({ schedule: 'manual' });
      const matches = screen.getAllByText('autopilot.wizard.schedule.manual');
      expect(matches.length).toBeGreaterThanOrEqual(1);
      // The summary value renders as a <span> (the schedule button renders as a <div>).
      const summarySpan = matches.find((el) => el.tagName.toLowerCase() === 'span');
      expect(summarySpan).toBeInTheDocument();
    });
  });
});
