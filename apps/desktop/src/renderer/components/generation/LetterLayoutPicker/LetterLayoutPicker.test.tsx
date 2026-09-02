import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { LETTER_LAYOUT_IDS } from '@/lib/generate';

import { LetterLayoutPicker } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const opt = (id: string) => `${TEST_IDS.generation.letterLayoutOption}-${id}`;

describe('LetterLayoutPicker', () => {
  it('shows Classic selected (aria-checked) when value is undefined — the backend default', () => {
    render(<LetterLayoutPicker onChange={vi.fn()} />);
    expect(screen.getByTestId(opt('classic'))).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByTestId(opt('refined'))).toHaveAttribute('aria-checked', 'false');
    expect(screen.getByTestId(opt('banded'))).toHaveAttribute('aria-checked', 'false');
  });

  it('marks the option matching an explicit value as checked', () => {
    render(<LetterLayoutPicker value="refined" onChange={vi.fn()} />);
    expect(screen.getByTestId(opt('refined'))).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByTestId(opt('classic'))).toHaveAttribute('aria-checked', 'false');
  });

  it('fires onChange with the chosen layout id on click', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<LetterLayoutPicker onChange={onChange} />);

    await user.click(screen.getByTestId(opt('banded')));
    expect(onChange).toHaveBeenCalledWith('banded');
  });

  it('keeps exactly one tab stop (the selected option) for APG roving-tabindex', () => {
    render(<LetterLayoutPicker value="banded" onChange={vi.fn()} />);
    expect(screen.getByTestId(opt('banded'))).toHaveAttribute('tabindex', '0');
    expect(screen.getByTestId(opt('classic'))).toHaveAttribute('tabindex', '-1');
    expect(screen.getByTestId(opt('refined'))).toHaveAttribute('tabindex', '-1');
  });

  it('keeps the radiogroup programmatically focusable without giving it a tab stop', async () => {
    const user = userEvent.setup();
    render(<LetterLayoutPicker value="banded" onChange={vi.fn()} />);
    const group = screen.getByRole('radiogroup', { name: 'aiGenerate.letterLayout' });

    // Same pair as the roving-tabindex test above, from the other direction:
    // that one reads the attribute, this one reads its two CONSEQUENCES, so
    // neither deleting `tabIndex={-1}` nor raising it to `0` can pass both.
    // Tab BEFORE focusing: tabbing out of an already-focused group lands on the
    // option either way, which would make the `tabIndex={0}` half vacuous.
    await user.tab();
    expect(screen.getByTestId(opt('banded'))).toHaveFocus();
    expect(group).not.toHaveFocus();

    group.focus();
    expect(group).toHaveFocus();
  });

  it('advances selection with ArrowDown from the in-set current option', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    // value undefined → Classic is the in-set selection (index 0).
    render(<LetterLayoutPicker onChange={onChange} />);

    // Tab lands on the single tab stop (Classic), then ArrowDown moves to Refined.
    await user.tab();
    expect(screen.getByTestId(opt('classic'))).toHaveFocus();
    await user.keyboard('{ArrowDown}');
    expect(onChange).toHaveBeenLastCalledWith('refined');
  });

  it('exposes a labeled radiogroup', () => {
    render(<LetterLayoutPicker onChange={vi.fn()} />);
    expect(screen.getByRole('radiogroup', { name: 'aiGenerate.letterLayout' })).toBeInTheDocument();
  });

  // Registration guard. The picker renders its own `LAYOUTS` array while the
  // keyboard handler walks `LETTER_LAYOUT_IDS`, so a layout added to the id set
  // but not to the picker is invisible — and one added in a different position
  // makes ArrowDown jump to an option that isn't next on screen. Neither shows
  // up in any other test here.
  it('renders exactly one option per LETTER_LAYOUT_IDS, in the same order', () => {
    render(<LetterLayoutPicker onChange={vi.fn()} />);
    const rendered = screen.getAllByRole('radio').map((el) => el.getAttribute('data-testid'));
    expect(rendered).toEqual(LETTER_LAYOUT_IDS.map((id) => opt(id)));
  });
});
