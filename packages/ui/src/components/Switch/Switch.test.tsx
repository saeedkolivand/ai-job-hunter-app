import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Switch, type SwitchProps } from './Switch';

function Harness({
  initial = false,
  onCheckedChange,
  ...rest
}: { initial?: boolean } & Partial<Omit<SwitchProps, 'checked'>>) {
  const [checked, setChecked] = useState(initial);
  return (
    <Switch
      aria-label="Toggle"
      {...rest}
      checked={checked}
      onCheckedChange={(v) => {
        setChecked(v);
        onCheckedChange?.(v);
      }}
    />
  );
}

describe('Switch', () => {
  it('renders role="switch" with aria-checked reflecting checked', () => {
    const { rerender } = render(
      <Switch aria-label="Toggle" checked={false} onCheckedChange={() => {}} />
    );
    const sw = screen.getByRole('switch', { name: 'Toggle' });
    expect(sw).toBeInTheDocument();
    expect(sw).toHaveAttribute('aria-checked', 'false');

    rerender(<Switch aria-label="Toggle" checked onCheckedChange={() => {}} />);
    expect(screen.getByRole('switch', { name: 'Toggle' })).toHaveAttribute('aria-checked', 'true');
  });

  it('calls onCheckedChange with !checked when clicked', async () => {
    const user = userEvent.setup();
    const onCheckedChange = vi.fn();
    render(<Switch aria-label="Toggle" checked={false} onCheckedChange={onCheckedChange} />);

    await user.click(screen.getByRole('switch', { name: 'Toggle' }));

    expect(onCheckedChange).toHaveBeenCalledTimes(1);
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });

  it('passes the negated value when already checked', async () => {
    const user = userEvent.setup();
    const onCheckedChange = vi.fn();
    render(<Switch aria-label="Toggle" checked onCheckedChange={onCheckedChange} />);

    await user.click(screen.getByRole('switch', { name: 'Toggle' }));

    expect(onCheckedChange).toHaveBeenCalledWith(false);
  });

  it('does not fire onCheckedChange when disabled', async () => {
    const user = userEvent.setup();
    const onCheckedChange = vi.fn();
    render(
      <Switch aria-label="Toggle" checked={false} disabled onCheckedChange={onCheckedChange} />
    );

    const sw = screen.getByRole('switch', { name: 'Toggle' });
    expect(sw).toBeDisabled();
    await user.click(sw);

    expect(onCheckedChange).not.toHaveBeenCalled();
  });

  it('renders label and description when label is provided', () => {
    render(
      <Switch
        label="Reduce transparency"
        description="Use opaque surfaces"
        checked={false}
        onCheckedChange={() => {}}
      />
    );
    expect(screen.getByText('Reduce transparency')).toBeInTheDocument();
    expect(screen.getByText('Use opaque surfaces')).toBeInTheDocument();
    // The visible <label htmlFor> supplies the accessible name in label mode.
    expect(screen.getByRole('switch', { name: 'Reduce transparency' })).toBeInTheDocument();
  });

  it('does not render a description when none is provided', () => {
    render(<Switch label="Just a label" checked={false} onCheckedChange={() => {}} />);
    expect(screen.getByText('Just a label')).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Just a label' })).toBeInTheDocument();
  });

  it('the visible label supplies the accessible name in label mode (aria-label dropped)', () => {
    render(
      <Switch
        label="Visible label"
        aria-label="Accessible name"
        checked={false}
        onCheckedChange={() => {}}
      />
    );
    // In label mode the <label htmlFor> association wins; the redundant aria-label
    // override is intentionally dropped, so the name is the visible label text.
    expect(screen.getByRole('switch', { name: 'Visible label' })).toBeInTheDocument();
  });

  it('toggles when the visible label text is clicked (whole-control click)', async () => {
    const user = userEvent.setup();
    const onCheckedChange = vi.fn();
    render(
      <Switch label="Reduce transparency" checked={false} onCheckedChange={onCheckedChange} />
    );

    await user.click(screen.getByText('Reduce transparency'));

    expect(onCheckedChange).toHaveBeenCalledTimes(1);
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });

  it('associates the description via aria-describedby', () => {
    render(
      <Switch
        label="Reduce transparency"
        description="Use opaque surfaces"
        checked={false}
        onCheckedChange={() => {}}
      />
    );
    const sw = screen.getByRole('switch', { name: 'Reduce transparency' });
    const descEl = screen.getByText('Use opaque surfaces');
    // The button points at the description element's id so AT announces the hint.
    expect(descEl.id).toBeTruthy();
    expect(sw).toHaveAttribute('aria-describedby', descEl.id);
  });

  it('honors the id prop on the switch button', () => {
    render(<Switch aria-label="Toggle" id="x" checked={false} onCheckedChange={() => {}} />);
    expect(screen.getByRole('switch', { name: 'Toggle' })).toHaveAttribute('id', 'x');
  });

  it('applies the md track classes by default', () => {
    render(<Switch aria-label="Toggle" checked={false} onCheckedChange={() => {}} />);
    const sw = screen.getByRole('switch', { name: 'Toggle' });
    expect(sw).toHaveClass('h-5', 'w-9');
  });

  it('applies the sm track classes when size="sm"', () => {
    render(<Switch aria-label="Toggle" size="sm" checked={false} onCheckedChange={() => {}} />);
    const sw = screen.getByRole('switch', { name: 'Toggle' });
    expect(sw).toHaveClass('h-4', 'w-7');
  });

  it('anchors the thumb with left-0 and slides via translate in both states (md)', () => {
    const { rerender } = render(
      <Switch aria-label="Toggle" checked={false} onCheckedChange={() => {}} />
    );
    const offThumb = screen.getByRole('switch', { name: 'Toggle' }).querySelector('span');
    expect(offThumb).not.toBeNull();
    // Anchor pins the thumb's left edge to the track padding box; translate slides it.
    expect(offThumb).toHaveClass('left-0', 'translate-x-0.5');
    expect(offThumb).not.toHaveClass('translate-x-4.5');

    rerender(<Switch aria-label="Toggle" checked onCheckedChange={() => {}} />);
    const onThumb = screen.getByRole('switch', { name: 'Toggle' }).querySelector('span');
    expect(onThumb).not.toBeNull();
    expect(onThumb).toHaveClass('left-0', 'translate-x-4.5');
    expect(onThumb).not.toHaveClass('translate-x-0.5');
  });

  it('anchors the thumb with left-0 and slides via translate in both states (sm)', () => {
    const { rerender } = render(
      <Switch aria-label="Toggle" size="sm" checked={false} onCheckedChange={() => {}} />
    );
    const off = screen.getByRole('switch', { name: 'Toggle' });
    // sm track matches antd small (28×16).
    expect(off).toHaveClass('h-4', 'w-7');
    const offThumb = off.querySelector('span');
    expect(offThumb).not.toBeNull();
    expect(offThumb).toHaveClass('left-0', 'translate-x-0.5');
    expect(offThumb).not.toHaveClass('translate-x-3.5');

    rerender(<Switch aria-label="Toggle" size="sm" checked onCheckedChange={() => {}} />);
    const onThumb = screen.getByRole('switch', { name: 'Toggle' }).querySelector('span');
    expect(onThumb).not.toBeNull();
    expect(onThumb).toHaveClass('left-0', 'translate-x-3.5');
    expect(onThumb).not.toHaveClass('translate-x-0.5');
  });

  it('toggles aria-checked through the controlled harness on click', async () => {
    const user = userEvent.setup();
    render(<Harness initial={false} />);
    const sw = screen.getByRole('switch', { name: 'Toggle' });
    expect(sw).toHaveAttribute('aria-checked', 'false');
    await user.click(sw);
    expect(screen.getByRole('switch', { name: 'Toggle' })).toHaveAttribute('aria-checked', 'true');
  });
});
