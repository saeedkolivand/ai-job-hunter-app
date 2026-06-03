import { createRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { NumberField } from './NumberField';

describe('NumberField', () => {
  it('renders the current value in the field', () => {
    render(<NumberField aria-label="count" value={42} onChange={vi.fn()} fallback={0} />);
    expect(screen.getByRole('spinbutton', { name: 'count' })).toHaveValue(42);
  });

  it('clear-to-empty: leaves the field empty and emits no onChange; typing then emits the new number', async () => {
    const onChange = vi.fn();
    render(<NumberField aria-label="count" value={5} onChange={onChange} fallback={0} />);
    const input = screen.getByRole('spinbutton', { name: 'count' });

    await userEvent.clear(input);
    expect(input).toHaveValue(null); // empty — NOT 0
    expect(onChange).not.toHaveBeenCalled();

    await userEvent.type(input, '5');
    expect(input).toHaveValue(5);
    expect(onChange).toHaveBeenLastCalledWith(5);
  });

  it('does not clamp mid-typing: typing a value above max emits the over-limit value while focused', async () => {
    const onChange = vi.fn();
    render(<NumberField aria-label="count" value={0} onChange={onChange} max={10} fallback={0} />);
    const input = screen.getByRole('spinbutton', { name: 'count' });

    await userEvent.clear(input);
    await userEvent.type(input, '15');
    expect(onChange).toHaveBeenLastCalledWith(15);
    // Input should still show 15 while the field is focused (no blur yet)
    expect(input).toHaveValue(15);
  });

  // -- Blur emissions (each asserts exactly one emission) ----------------------

  it('blur with empty field: displays fallback and emits fallback exactly once', async () => {
    const onChange = vi.fn();
    render(<NumberField aria-label="count" value={5} onChange={onChange} fallback={1} />);
    const input = screen.getByRole('spinbutton', { name: 'count' });

    await userEvent.clear(input);
    expect(onChange).not.toHaveBeenCalled();

    onChange.mockClear();
    await userEvent.tab(); // trigger blur
    expect(input).toHaveValue(1);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(1);
  });

  it('blur above max: clamps to max and emits max exactly once', async () => {
    const onChange = vi.fn();
    render(
      <NumberField aria-label="count" value={5} onChange={onChange} min={1} max={10} fallback={1} />
    );
    const input = screen.getByRole('spinbutton', { name: 'count' });

    await userEvent.clear(input);
    await userEvent.type(input, '99');
    onChange.mockClear();
    await userEvent.tab();

    expect(input).toHaveValue(10);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(10);
  });

  it('blur below min: clamps to min and emits min exactly once', async () => {
    const onChange = vi.fn();
    render(
      <NumberField aria-label="count" value={5} onChange={onChange} min={1} max={10} fallback={1} />
    );
    const input = screen.getByRole('spinbutton', { name: 'count' });

    await userEvent.clear(input);
    await userEvent.type(input, '0');
    onChange.mockClear();
    await userEvent.tab();

    expect(input).toHaveValue(1);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(1);
  });

  it('blur within range: emits the unchanged value exactly once', async () => {
    const onChange = vi.fn();
    render(
      <NumberField aria-label="count" value={5} onChange={onChange} min={1} max={10} fallback={1} />
    );
    const input = screen.getByRole('spinbutton', { name: 'count' });

    await userEvent.clear(input);
    await userEvent.type(input, '5');
    onChange.mockClear();
    await userEvent.tab();

    expect(input).toHaveValue(5);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(5);
  });

  it('NaN buffer on blur: falls back to fallback and emits fallback exactly once', async () => {
    const onChange = vi.fn();
    render(<NumberField aria-label="count" value={5} onChange={onChange} fallback={3} />);
    const input = screen.getByRole('spinbutton', { name: 'count' });

    // userEvent.type won't type letters into a number input, so drive the
    // change event directly to force a NaN buffer.
    fireEvent.change(input, { target: { value: 'abc' } });
    expect(onChange).not.toHaveBeenCalled();

    onChange.mockClear();
    fireEvent.blur(input);

    expect(input).toHaveValue(3);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(3);
  });

  // -- Resync (external value prop) -------------------------------------------

  it('external resync: rerender with a new value prop updates the displayed buffer without calling onChange', () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <NumberField aria-label="count" value={3} onChange={onChange} fallback={0} />
    );
    expect(screen.getByRole('spinbutton', { name: 'count' })).toHaveValue(3);

    onChange.mockClear();
    rerender(<NumberField aria-label="count" value={99} onChange={onChange} fallback={0} />);
    expect(screen.getByRole('spinbutton', { name: 'count' })).toHaveValue(99);
    // Resync must NOT emit onChange (infinite-loop guard)
    expect(onChange).not.toHaveBeenCalled();
  });

  // -- Negative numbers --------------------------------------------------------

  it('negative numbers: clearing then typing -5 emits -5 (no min set)', async () => {
    const onChange = vi.fn();
    render(<NumberField aria-label="count" value={0} onChange={onChange} fallback={0} />);
    const input = screen.getByRole('spinbutton', { name: 'count' });

    await userEvent.clear(input);
    await userEvent.type(input, '-5');
    expect(onChange).toHaveBeenLastCalledWith(-5);
    expect(input).toHaveValue(-5);
  });

  it('negative numbers: a lone "-" mid-type does not emit onChange', async () => {
    const onChange = vi.fn();
    render(<NumberField aria-label="count" value={0} onChange={onChange} fallback={0} />);
    const input = screen.getByRole('spinbutton', { name: 'count' });

    await userEvent.clear(input);
    // Drive a raw change so the buffer holds just "-" (not parseable to a number)
    fireEvent.change(input, { target: { value: '-' } });
    expect(onChange).not.toHaveBeenCalled();
  });

  // -- Pass-through attributes -------------------------------------------------

  it('passes through aria-label, className, disabled, and step', () => {
    render(
      <NumberField
        aria-label="seats"
        value={2}
        onChange={vi.fn()}
        fallback={0}
        disabled
        step={0.5}
        className="extra-class"
      />
    );
    const input = screen.getByRole('spinbutton', { name: 'seats' });
    expect(input).toBeDisabled();
    expect(input.className).toContain('extra-class');
    expect(input).toHaveAttribute('step', '0.5');
  });

  // -- onBlur forwarding -------------------------------------------------------

  it('forwards a caller-supplied onBlur with the blur event after its own handler runs', async () => {
    const onBlur = vi.fn();
    render(
      <NumberField aria-label="amount" value={3} onChange={vi.fn()} fallback={0} onBlur={onBlur} />
    );
    const input = screen.getByRole('spinbutton', { name: 'amount' });
    await userEvent.click(input);
    await userEvent.tab(); // triggers blur → internal handleBlur → rest.onBlur?.(e)

    expect(onBlur).toHaveBeenCalledTimes(1);
    // The forwarded argument must be the actual FocusEvent, not undefined
    expect(onBlur).toHaveBeenCalledWith(expect.objectContaining({ type: 'blur' }));
  });

  // -- ref forwarding ----------------------------------------------------------

  it('forwards ref to the underlying input element', () => {
    const ref = createRef<HTMLInputElement>();
    render(<NumberField aria-label="count" value={7} onChange={vi.fn()} fallback={0} ref={ref} />);

    expect(ref.current).not.toBeNull();
    expect(ref.current?.tagName).toBe('INPUT');
    // Ref must point at the same DOM node that testing-library resolves
    expect(ref.current).toBe(screen.getByRole('spinbutton', { name: 'count' }));
  });
});
