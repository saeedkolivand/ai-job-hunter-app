import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { SegmentedControl, type SegmentedOption } from './SegmentedControl';

type Quality = 'full' | 'auto' | 'compact';

const OPTIONS: readonly SegmentedOption<Quality>[] = [
  { value: 'full', label: 'Full' },
  { value: 'auto', label: 'Auto' },
  { value: 'compact', label: 'Fast' },
];

function Harness({
  initial = 'auto' as Quality,
  onChange,
}: {
  initial?: Quality;
  onChange?: (v: Quality) => void;
}) {
  const [value, setValue] = useState<Quality>(initial);
  return (
    <SegmentedControl
      ariaLabel="Prompt quality"
      options={OPTIONS}
      value={value}
      onChange={(v) => {
        setValue(v);
        onChange?.(v);
      }}
    />
  );
}

describe('SegmentedControl', () => {
  it('renders a radiogroup with one checked radio reflecting value', () => {
    render(<Harness initial="auto" />);
    expect(screen.getByRole('radiogroup', { name: 'Prompt quality' })).toBeInTheDocument();
    const radios = screen.getAllByRole('radio');
    expect(radios).toHaveLength(3);
    expect(screen.getByRole('radio', { name: 'Auto' })).toBeChecked();
    expect(screen.getByRole('radio', { name: 'Full' })).not.toBeChecked();
  });

  it('only the selected radio is in the tab order (roving tabindex)', () => {
    render(<Harness initial="auto" />);
    expect(screen.getByRole('radio', { name: 'Auto' })).toHaveAttribute('tabindex', '0');
    expect(screen.getByRole('radio', { name: 'Full' })).toHaveAttribute('tabindex', '-1');
  });

  it('fires onChange when a radio is clicked', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness initial="auto" onChange={onChange} />);
    await user.click(screen.getByRole('radio', { name: 'Fast' }));
    expect(onChange).toHaveBeenCalledWith('compact');
    expect(screen.getByRole('radio', { name: 'Fast' })).toBeChecked();
  });

  it('moves selection with arrow keys and wraps at the ends', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness initial="auto" onChange={onChange} />);
    const auto = screen.getByRole('radio', { name: 'Auto' });
    auto.focus();
    await user.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenLastCalledWith('compact');
    expect(screen.getByRole('radio', { name: 'Fast' })).toBeChecked();
    // wrap forward: compact -> full
    await user.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenLastCalledWith('full');
    // End jumps to last
    await user.keyboard('{End}');
    expect(onChange).toHaveBeenLastCalledWith('compact');
    // Home jumps back to first
    await user.keyboard('{Home}');
    expect(onChange).toHaveBeenLastCalledWith('full');
  });

  it('renders the grid variant with explicit column count', () => {
    render(
      <SegmentedControl
        ariaLabel="Quality"
        variant="grid"
        options={OPTIONS}
        value="full"
        onChange={() => {}}
      />
    );
    const group = screen.getByRole('radiogroup', { name: 'Quality' });
    expect(group.style.gridTemplateColumns).toBe('repeat(3, minmax(0, 1fr))');
  });
});
