import { type ComponentProps, forwardRef, useEffect, useRef, useState } from 'react';

import { Input } from '../Input';

export interface NumberFieldProps extends Omit<
  ComponentProps<typeof Input>,
  'type' | 'value' | 'onChange' | 'min' | 'max' | 'step'
> {
  /** Current numeric value (controlled). */
  value: number;
  /**
   * Emitted when the buffer parses to a finite number, and again (clamped) on
   * blur. Clamping to `[min, max]` is blur-only, so values emitted **while
   * typing may fall outside `[min, max]`** — callers persisting the value must
   * not assume it is in-range mid-edit (it is reconciled on blur).
   */
  onChange: (value: number) => void;
  /** Lower clamp bound, applied on blur only. */
  min?: number;
  /** Upper clamp bound, applied on blur only. */
  max?: number;
  /** Native step. */
  step?: number;
  /** Value to fall back to when the field is left empty / invalid on blur. */
  fallback: number;
}

const clamp = (n: number, min?: number, max?: number): number => {
  let out = n;
  if (typeof min === 'number') out = Math.max(min, out);
  if (typeof max === 'number') out = Math.min(max, out);
  return out;
};

/**
 * Numeric input that keeps an internal string buffer so the field can be empty
 * or mid-edit while the user types — fixing the "can't clear the 0" bug where
 * `Number('') === 0` snaps controlled inputs back to `0` and traps the cursor.
 *
 * - While typing: emits `onChange` only when the buffer parses to a finite
 *   number; never clamps mid-typing so intermediate values aren't blocked.
 *   Because clamping is blur-only, mid-edit `onChange` values may be outside
 *   `[min, max]` — don't treat the value as in-range until blur.
 * - On blur: empty/NaN → `fallback`; otherwise clamps to `[min, max]`.
 * - Resyncs the buffer when the external `value` changes (e.g. a prefill).
 *
 * Forwards `ref` to the underlying `<input>` element.
 */
export const NumberField = forwardRef<HTMLInputElement, NumberFieldProps>(function NumberField(
  { value, onChange, min, max, step, fallback, ...rest },
  ref
) {
  const [buffer, setBuffer] = useState(() => String(value));
  // Tracks the numeric value the buffer currently represents, so the resync
  // effect can compare against an external `value` change without depending on
  // (and re-running for) every keystroke.
  const lastValueRef = useRef(value);

  // Resync when the controlled value changes externally (prefill, "use
  // suggested", programmatic reset) to something the buffer isn't reflecting.
  useEffect(() => {
    if (value !== lastValueRef.current) {
      lastValueRef.current = value;
      setBuffer(String(value));
    }
  }, [value]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const next = e.target.value;
    setBuffer(next);
    if (next.trim() === '') return; // allow empty while typing — don't emit
    const parsed = Number(next);
    if (Number.isFinite(parsed)) {
      lastValueRef.current = parsed;
      onChange(parsed);
    }
  };

  const handleBlur = (e: React.FocusEvent<HTMLInputElement>) => {
    const parsed = Number(buffer);
    if (buffer.trim() === '' || !Number.isFinite(parsed)) {
      setBuffer(String(fallback));
      lastValueRef.current = fallback;
      onChange(fallback);
    } else {
      const clamped = clamp(parsed, min, max);
      setBuffer(String(clamped));
      lastValueRef.current = clamped;
      onChange(clamped);
    }
    rest.onBlur?.(e);
  };

  return (
    <Input
      {...rest}
      ref={ref}
      type="number"
      min={min}
      max={max}
      step={step}
      value={buffer}
      onChange={handleChange}
      onBlur={handleBlur}
    />
  );
});

NumberField.displayName = 'NumberField';
