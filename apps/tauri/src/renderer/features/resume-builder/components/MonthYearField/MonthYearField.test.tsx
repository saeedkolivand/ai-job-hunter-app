/**
 * MonthYearField — render and interaction tests.
 *
 * SelectDropdown is a real custom dropdown backed by a trigger `<button>` and a
 * portal `role="listbox"` with `role="option"` items. We drive it with
 * userEvent (click trigger → click option) the same way SelectDropdown's own
 * test suite does.
 *
 * Covers:
 *  - Renders two trigger buttons (month + year dropdowns) when `present` is false.
 *  - Selecting a month then a year emits the combined "MMM YYYY" string.
 *  - Selecting only one of the two fields emits '' (both required to form a value).
 *  - Clearing a field (selecting when only one chosen) emits ''.
 *  - When `present` is true the read-only "Present" span is shown and the dropdowns
 *    are absent.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string) => k,
    i18n: { language: 'en-US' },
  }),
}));

import { MonthYearField } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderField(props: {
  value: string;
  onChange: (v: string) => void;
  present?: boolean;
  disabled?: boolean;
}) {
  return render(<MonthYearField {...props} />);
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('MonthYearField — default (not present)', () => {
  it('renders two dropdown trigger buttons', () => {
    renderField({ value: '', onChange: () => {} });
    // SelectDropdown renders a <button aria-haspopup="listbox"> for each dropdown.
    const triggers = screen.getAllByRole('button', { hidden: false });
    expect(triggers.length).toBeGreaterThanOrEqual(2);
  });

  it('shows month placeholder when value is empty', () => {
    renderField({ value: '', onChange: () => {} });
    expect(screen.getByText('build.monthYear.month')).toBeInTheDocument();
  });

  it('shows year placeholder when value is empty', () => {
    renderField({ value: '', onChange: () => {} });
    expect(screen.getByText('build.monthYear.year')).toBeInTheDocument();
  });

  it('displays the month token from a pre-set value', () => {
    // "Jan 2020" → month trigger should show the label for Jan
    renderField({ value: 'Jan 2020', onChange: () => {} });
    // The trigger text is the label from monthOptions; in en-US locale "Jan" maps to "Jan"
    expect(screen.getAllByText(/Jan/i).length).toBeGreaterThanOrEqual(1);
  });

  it('displays the year from a pre-set value', () => {
    renderField({ value: 'Jan 2020', onChange: () => {} });
    expect(screen.getAllByText('2020').length).toBeGreaterThanOrEqual(1);
  });
});

describe('MonthYearField — selecting values', () => {
  let onChange: ReturnType<typeof vi.fn<(value: string) => void>>;

  beforeEach(() => {
    onChange = vi.fn<(value: string) => void>();
  });

  it('emits combined "MMM YYYY" when both month and year are selected', async () => {
    // Start with a valid pre-set value so both month (Jan) and year (2022) are
    // already parsed.  Selecting a different month (Mar) while year is '2022'
    // → emit('Mar', '2022') → 'Mar 2022'.
    renderField({ value: 'Jan 2022', onChange });

    // Open the month dropdown (first trigger button).
    const [monthTrigger] = screen.getAllByRole('button');
    expect(monthTrigger).toBeDefined();
    await userEvent.click(monthTrigger as HTMLElement);

    // Pick "Mar" from the listbox options.
    const marOption = await screen.findByRole('option', { name: /^Mar$/ });
    await userEvent.click(marOption);

    expect(onChange).toHaveBeenCalledWith('Mar 2022');
  });

  it('emits empty string when month is selected but year is absent', async () => {
    // value is '' → both blank. After picking a month without a year → emit('Jan', '') → ''
    renderField({ value: '', onChange });

    const [monthTrigger] = screen.getAllByRole('button');
    expect(monthTrigger).toBeDefined();
    await userEvent.click(monthTrigger as HTMLElement);

    const janOption = await screen.findByRole('option', { name: /Jan/i });
    await userEvent.click(janOption);

    expect(onChange).toHaveBeenCalledWith('');
  });
});

describe('MonthYearField — present mode', () => {
  it('renders the i18n key text for "Present"', () => {
    renderField({ value: '', onChange: () => {}, present: true });
    expect(screen.getByText('build.experience.present')).toBeInTheDocument();
  });

  it('does not render dropdown trigger buttons when present is true', () => {
    renderField({ value: '', onChange: () => {}, present: true });
    // No listbox triggers should exist.
    expect(screen.queryAllByRole('button').length).toBe(0);
  });
});
