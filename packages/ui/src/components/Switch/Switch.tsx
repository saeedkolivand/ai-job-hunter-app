import { useId } from 'react';

import { cn } from '../../lib/cn';
import { Button } from '../Button';

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (next: boolean) => void;
  /** Track density. Default `md`. */
  size?: 'sm' | 'md';
  /** When set, renders a row: label/description on the left, switch on the right. */
  label?: string;
  /** Optional sub-text under the label. Only rendered when `label` is set. */
  description?: string;
  disabled?: boolean;
  /** Accessible name for the switch when no `label` is provided. */
  'aria-label'?: string;
  /** Set the switch button's id so an external `<label htmlFor>` can target it. */
  id?: string;
  /** Applied to the switch track element. */
  className?: string;
}

const TRACK_BASE =
  'relative shrink-0 cursor-pointer rounded-full border-transparent p-0 transition-colors focus-visible:ring-2 focus-visible:ring-brand/50 disabled:cursor-not-allowed';
const THUMB_BASE = 'absolute left-0 top-0.5 rounded-full bg-white shadow-sm transition-transform';

const SIZE_TRACK: Record<NonNullable<SwitchProps['size']>, string> = {
  sm: 'h-4 w-7',
  md: 'h-5 w-9',
};
const SIZE_THUMB: Record<NonNullable<SwitchProps['size']>, string> = {
  sm: 'h-3 w-3',
  md: 'h-4 w-4',
};
const TRANSLATE_ON: Record<NonNullable<SwitchProps['size']>, string> = {
  sm: 'translate-x-3.5',
  md: 'translate-x-4.5',
};
const TRANSLATE_OFF = 'translate-x-0.5';

/**
 * Boolean toggle rendered as `role="switch"`, built on `Button variant="unstyled"`.
 * Centralises the track/thumb markup that was hand-rolled across settings + the
 * Autopilot ATS chip so the on/off colour, sizing, and focus-ring live in one
 * place. Pass `label` to get the standard settings row (label/description left,
 * switch right) — the label is a real `<label htmlFor>` so clicking it toggles
 * the switch and supplies the accessible name; otherwise the bare switch is
 * returned and `aria-label` supplies the accessible name.
 */
export function Switch({
  checked,
  onCheckedChange,
  size = 'md',
  label,
  description,
  disabled,
  'aria-label': ariaLabel,
  id,
  className,
}: SwitchProps) {
  const reactId = useId();
  const switchId = id ?? reactId;
  const descId = `${switchId}-desc`;

  const switchEl = (
    <Button
      id={switchId}
      variant="unstyled"
      type="button"
      role="switch"
      aria-checked={checked}
      // In label mode the visible `<label htmlFor>` supplies the accessible name,
      // so we drop aria-label to avoid a redundant override.
      aria-label={label ? undefined : ariaLabel}
      aria-describedby={description ? descId : undefined}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        TRACK_BASE,
        SIZE_TRACK[size],
        checked ? 'bg-brand' : 'bg-foreground/15',
        className
      )}
    >
      <span
        className={cn(THUMB_BASE, SIZE_THUMB[size], checked ? TRANSLATE_ON[size] : TRANSLATE_OFF)}
      />
    </Button>
  );

  if (!label) return switchEl;

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="min-w-0">
        {/* The button is a labelable element, so htmlFor associates this label with
            it: clicking the text toggles the switch (whole-control click).
            The label wraps ONLY the name text — the description stays outside so the
            accessible name is just the label, with the hint exposed once via
            aria-describedby (not concatenated into the name). */}
        <label
          htmlFor={switchId}
          className="block cursor-pointer select-none text-xs font-medium text-foreground/80"
        >
          {label}
        </label>
        {description && (
          <div id={descId} className="text-[11px] text-foreground/45">
            {description}
          </div>
        )}
      </div>
      {switchEl}
    </div>
  );
}
