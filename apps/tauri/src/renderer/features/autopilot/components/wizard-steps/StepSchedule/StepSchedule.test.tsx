import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { SetFn, WizardState } from '@/features/autopilot/types';

import { StepSchedule } from './index';

// ── Module stubs ──────────────────────────────────────────────────────────────

// Return every translation key verbatim — no i18next setup needed in jsdom.
vi.mock('@/lib/i18n', () => ({
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
    pages: 2,
    dateFilter: '24h',
    minMatchScore: 50,
    keywords: '',
    excludeKeywords: '',
    resumeText: '',
    coverLetter: '',
    schedule: 'daily',
    scheduleHour: 9,
    scheduleMinute: 0,
    ...overrides,
  };
}

/** Open the SelectDropdown identified by its trigger button id attribute and pick an option. */
async function pickOptionById(
  user: ReturnType<typeof userEvent.setup>,
  triggerId: string,
  optionLabel: string
) {
  // SelectDropdown forwards an `id` onto its trigger button.
  const trigger = document.getElementById(triggerId);
  if (!trigger) throw new Error(`No element with id="${triggerId}"`);
  await user.click(trigger);
  expect(screen.getByRole('listbox')).toBeInTheDocument();
  await user.click(screen.getByRole('option', { name: optionLabel }));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('StepSchedule', () => {
  let set: Mock<SetFn>;

  beforeEach(() => {
    set = vi.fn<SetFn>();
  });

  // ── Gating: control visibility per schedule ─────────────────────────────────

  describe('gating — daily', () => {
    it('renders both hour and minute controls', () => {
      render(<StepSchedule form={makeForm({ schedule: 'daily' })} set={set} />);
      expect(document.getElementById('schedule-hour')).toBeInTheDocument();
      expect(document.getElementById('schedule-minute')).toBeInTheDocument();
    });

    it('does not render the minutes-past-hour control', () => {
      render(<StepSchedule form={makeForm({ schedule: 'daily' })} set={set} />);
      expect(document.getElementById('schedule-minutes-past-hour')).not.toBeInTheDocument();
    });
  });

  describe('gating — twice_daily', () => {
    it('renders both hour and minute controls', () => {
      render(<StepSchedule form={makeForm({ schedule: 'twice_daily' })} set={set} />);
      expect(document.getElementById('schedule-hour')).toBeInTheDocument();
      expect(document.getElementById('schedule-minute')).toBeInTheDocument();
    });

    it('shows a "+12h" hint containing the second run time (hour 9 → 21:00)', () => {
      render(
        <StepSchedule
          form={makeForm({ schedule: 'twice_daily', scheduleHour: 9, scheduleMinute: 0 })}
          set={set}
        />
      );
      // The hint text contains "21:00" — the first run at 09:00 plus 12h.
      expect(screen.getByText(/21:00/)).toBeInTheDocument();
    });

    it('computes the second run time correctly for hour 13 → 01:00 (wraps at midnight)', () => {
      render(
        <StepSchedule
          form={makeForm({ schedule: 'twice_daily', scheduleHour: 13, scheduleMinute: 15 })}
          set={set}
        />
      );
      // (13 + 12) % 24 = 1 → "01:15"
      expect(screen.getByText(/01:15/)).toBeInTheDocument();
    });

    it('scheduleHour=0 boundary: "+12h" hint contains "12:00" and hour trigger shows "00" not the placeholder', async () => {
      const user = userEvent.setup();
      render(
        <StepSchedule
          form={makeForm({ schedule: 'twice_daily', scheduleHour: 0, scheduleMinute: 0 })}
          set={set}
        />
      );

      // (a) The "+12h" hint must contain "12:00" — (0+12)%24 = 12, minute 0.
      // The hint is rendered by the alsoRunsAt i18n key which interpolates first/second.
      // Because our mock substitutes {{second}} → "12:00" the text "12:00" appears in the DOM.
      expect(screen.getByText(/12:00/)).toBeInTheDocument();

      // (b) The hour trigger's displayed label must be "00", not the placeholder "Select…".
      // SelectDropdown looks up `options.find(o => o.value === value)` and renders
      // `selectedOption.label`. For scheduleHour=0, value prop = String(0) = "0" which
      // matches HOUR_OPTIONS[0].value ("0") → label "00". A naive falsy guard on value
      // would fall through to the placeholder instead.
      const hourTrigger = document.getElementById('schedule-hour');
      if (!hourTrigger) throw new Error('schedule-hour trigger not found');

      // Open the dropdown and confirm the "00" option is marked as selected (aria-selected).
      await user.click(hourTrigger);
      expect(screen.getByRole('listbox')).toBeInTheDocument();
      const zeroOption = screen.getByRole('option', { name: '00' });
      expect(zeroOption).toHaveAttribute('aria-selected', 'true');

      // Also confirm the trigger itself displays "00" and not the placeholder text.
      // The trigger <button> wraps a <span> whose text is selectedOption.label.
      expect(hourTrigger).toHaveTextContent('00');
      expect(hourTrigger).not.toHaveTextContent('Select…');
    });
  });

  describe('gating — hourly', () => {
    it('renders the minutes-past-hour control', () => {
      render(<StepSchedule form={makeForm({ schedule: 'hourly' })} set={set} />);
      expect(document.getElementById('schedule-minutes-past-hour')).toBeInTheDocument();
    });

    it('does not render the hour control', () => {
      render(<StepSchedule form={makeForm({ schedule: 'hourly' })} set={set} />);
      expect(document.getElementById('schedule-hour')).not.toBeInTheDocument();
    });
  });

  describe('gating — manual', () => {
    it('renders no time controls at all', () => {
      render(<StepSchedule form={makeForm({ schedule: 'manual' })} set={set} />);
      expect(document.getElementById('schedule-hour')).not.toBeInTheDocument();
      expect(document.getElementById('schedule-minute')).not.toBeInTheDocument();
      expect(document.getElementById('schedule-minutes-past-hour')).not.toBeInTheDocument();
    });
  });

  // ── Wiring: set() called with correct type ──────────────────────────────────

  describe('wiring — hour control', () => {
    it('calls set("scheduleHour", <number>) when a new hour is selected', async () => {
      const user = userEvent.setup();
      render(<StepSchedule form={makeForm({ schedule: 'daily', scheduleHour: 9 })} set={set} />);
      await pickOptionById(user, 'schedule-hour', '08');
      expect(set).toHaveBeenCalledWith('scheduleHour', 8);
      // Guard: value must be a number, not a string.
      const [, value] = set.mock.calls.find(([k]) => k === 'scheduleHour') ?? [];
      expect(typeof value).toBe('number');
    });
  });

  describe('wiring — minute control (daily)', () => {
    it('calls set("scheduleMinute", <number>) when a new minute is selected', async () => {
      const user = userEvent.setup();
      render(<StepSchedule form={makeForm({ schedule: 'daily', scheduleMinute: 0 })} set={set} />);
      await pickOptionById(user, 'schedule-minute', '15');
      expect(set).toHaveBeenCalledWith('scheduleMinute', 15);
      const [, value] = set.mock.calls.find(([k]) => k === 'scheduleMinute') ?? [];
      expect(typeof value).toBe('number');
    });
  });

  describe('wiring — minute control (hourly)', () => {
    it('calls set("scheduleMinute", <number>) for the minutes-past-hour control', async () => {
      const user = userEvent.setup();
      render(<StepSchedule form={makeForm({ schedule: 'hourly', scheduleMinute: 0 })} set={set} />);
      await pickOptionById(user, 'schedule-minutes-past-hour', '30');
      expect(set).toHaveBeenCalledWith('scheduleMinute', 30);
      const [, value] = set.mock.calls.find(([k]) => k === 'scheduleMinute') ?? [];
      expect(typeof value).toBe('number');
    });
  });

  describe('wiring — schedule selector buttons', () => {
    it('calls set("schedule", "hourly") when the hourly button is clicked', async () => {
      const user = userEvent.setup();
      render(<StepSchedule form={makeForm({ schedule: 'daily' })} set={set} />);
      await user.click(
        screen.getByRole('button', { name: /autopilot\.wizard\.schedule\.hourly/i })
      );
      expect(set).toHaveBeenCalledWith('schedule', 'hourly');
    });

    it('calls set("schedule", "manual") when the manual button is clicked', async () => {
      const user = userEvent.setup();
      render(<StepSchedule form={makeForm({ schedule: 'daily' })} set={set} />);
      await user.click(
        screen.getByRole('button', { name: /autopilot\.wizard\.schedule\.manual/i })
      );
      expect(set).toHaveBeenCalledWith('schedule', 'manual');
    });
  });

  // ── Summary section ─────────────────────────────────────────────────────────

  describe('summary line', () => {
    it('daily: shows "HH:MM" formatted time (09:00 for hour 9 minute 0)', () => {
      render(
        <StepSchedule
          form={makeForm({ schedule: 'daily', scheduleHour: 9, scheduleMinute: 0 })}
          set={set}
        />
      );
      // The summary value cell renders the scheduleSummary string which includes "09:00".
      expect(screen.getByText(/09:00/)).toBeInTheDocument();
    });

    it('twice_daily: shows both times in the summary', () => {
      render(
        <StepSchedule
          form={makeForm({ schedule: 'twice_daily', scheduleHour: 9, scheduleMinute: 0 })}
          set={set}
        />
      );
      // Summary contains "09:00 & 21:00".
      expect(screen.getByText(/09:00.*21:00/s)).toBeInTheDocument();
    });

    it('hourly: shows ":MM" format (":00" for minute 0)', () => {
      render(<StepSchedule form={makeForm({ schedule: 'hourly', scheduleMinute: 0 })} set={set} />);
      expect(screen.getByText(/:00/)).toBeInTheDocument();
    });

    it('manual: shows the manual i18n key (no time component)', () => {
      render(<StepSchedule form={makeForm({ schedule: 'manual' })} set={set} />);
      // The summary value cell is a <span> rendered next to the "summarySchedule" label.
      // The same key also appears on the schedule selector button; getAllByText handles both.
      const matches = screen.getAllByText('autopilot.wizard.schedule.manual');
      // At least one match must exist (summary value); button label is the other.
      expect(matches.length).toBeGreaterThanOrEqual(1);
      // The summary value renders as a <span> (the schedule button renders as a <div>).
      const summarySpan = matches.find((el) => el.tagName.toLowerCase() === 'span');
      expect(summarySpan).toBeInTheDocument();
    });
  });
});
