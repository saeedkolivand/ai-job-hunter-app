import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { AccentPicker } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const navySwatch = `${TEST_IDS.generation.accentSwatch}-navy`;

describe('AccentPicker', () => {
  it('starts on "Template default" (aria-checked) when value is undefined', () => {
    render(<AccentPicker onChange={vi.fn()} />);
    expect(screen.getByTestId(TEST_IDS.generation.accentDefault)).toHaveAttribute(
      'aria-checked',
      'true'
    );
  });

  it('fires onChange with the swatch hex when a curated swatch is clicked', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<AccentPicker onChange={onChange} />);

    await user.click(screen.getByTestId(navySwatch));
    expect(onChange).toHaveBeenCalledWith('#1B3A5C');
  });

  it('marks the matching swatch active for a value that equals a curated hex', () => {
    render(<AccentPicker value="#1b3a5c" onChange={vi.fn()} />);
    // Case-insensitive match — the navy swatch is selected.
    expect(screen.getByTestId(navySwatch)).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByTestId(TEST_IDS.generation.accentDefault)).toHaveAttribute(
      'aria-checked',
      'false'
    );
  });

  it('clears the accent (undefined) when the default chip is clicked', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<AccentPicker value="#1B3A5C" onChange={onChange} />);

    await user.click(screen.getByTestId(TEST_IDS.generation.accentDefault));
    expect(onChange).toHaveBeenCalledWith(undefined);
  });

  it('propagates a valid 6-hex custom value (normalized to #RRGGBB uppercase)', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<AccentPicker onChange={onChange} />);

    await user.type(screen.getByTestId(TEST_IDS.generation.accentCustom), 'aabbcc');
    expect(onChange).toHaveBeenLastCalledWith('#AABBCC');
  });

  it('does NOT propagate a malformed custom value', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<AccentPicker onChange={onChange} />);

    const input = screen.getByTestId(TEST_IDS.generation.accentCustom);
    await user.type(input, 'zzz');
    expect(onChange).not.toHaveBeenCalled();
    // The field flags the invalid entry for the user.
    expect(input).toHaveAttribute('aria-invalid', 'true');
  });

  it('keeps the default chip tabbable and keyboard-reachable while a custom accent is active', async () => {
    const user = userEvent.setup();
    // A valid hex that matches NO curated swatch → the "custom" state.
    render(<AccentPicker value="#123456" onChange={vi.fn()} />);

    const def = screen.getByTestId(TEST_IDS.generation.accentDefault);
    // APG roving-tabindex: the radiogroup always keeps exactly one tab stop.
    // 'custom' is outside the radio set, so the default chip is the fallback.
    expect(def).toHaveAttribute('tabindex', '0');
    // It is only the tab fallback, not the selection — aria state stays false.
    expect(def).toHaveAttribute('aria-checked', 'false');
    // No swatch steals the tab stop while custom is active.
    expect(screen.getByTestId(navySwatch)).toHaveAttribute('tabindex', '-1');

    // Keyboard users can Tab into the group and land on the default chip.
    await user.tab();
    expect(def).toHaveFocus();
  });

  it('resets to the template default (undefined) when the custom input is cleared', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<AccentPicker value="#123456" onChange={onChange} />);

    await user.clear(screen.getByTestId(TEST_IDS.generation.accentCustom));
    // An emptied custom field is an explicit reset, not a stuck last value.
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });
});
