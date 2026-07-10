import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

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
});
