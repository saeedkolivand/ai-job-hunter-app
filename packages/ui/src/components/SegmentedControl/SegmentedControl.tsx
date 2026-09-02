import type { LucideIcon } from 'lucide-react';
import { type KeyboardEvent, type ReactNode, useRef } from 'react';

import { cn } from '../../lib/cn';

export interface SegmentedOption<T extends string> {
  value: T;
  label: ReactNode;
  /** Optional leading icon. */
  icon?: LucideIcon;
  /** Native title / tooltip. */
  title?: string;
  /**
   * Render the option but refuse selection — for a choice that exists in the
   * vocabulary and is not available yet (or not here). Skipped by arrow-key
   * roving and by click, and marked `aria-disabled` rather than `disabled` so it
   * stays reachable to a screen reader (a silently-missing option is not an
   * honest way to say "not available"; give it a `title` explaining why).
   */
  disabled?: boolean;
}

export interface SegmentedControlProps<T extends string> {
  options: readonly SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  /**
   * Visual genre:
   * - `track` (default) — iOS-style segmented control sitting in a tinted track.
   * - `grid` — equal-width bordered cards in a single row.
   */
  variant?: 'track' | 'grid';
  /** Track density. Ignored for `grid`. Default `md`. */
  size?: 'sm' | 'md';
  /** Active-fill family for the `track` variant. Default `neutral`. */
  tone?: 'brand' | 'neutral';
  /** Accessible name for the group (wired to `aria-label`). */
  ariaLabel?: string;
  className?: string;
}

const TRACK_ITEM_SIZE: Record<NonNullable<SegmentedControlProps<string>['size']>, string> = {
  sm: 'rounded-md px-2 py-1 text-[10px]',
  md: 'rounded-md px-3 py-1 text-[11px] font-medium',
};

const TRACK_ACTIVE: Record<NonNullable<SegmentedControlProps<string>['tone']>, string> = {
  brand: 'bg-brand/15 text-brand-soft',
  neutral: 'bg-foreground/10 text-foreground/90',
};

const ICON_SIZE = { sm: 11, md: 13 } as const;

/**
 * Single-select control rendered as a radio group. Covers the two segmented
 * patterns that recur across the app — the tinted iOS `track` and the bordered
 * `grid` — so the `role="radiogroup"` + roving arrow-key semantics live in one
 * place instead of being re-implemented per feature.
 */
export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  variant = 'track',
  size = 'md',
  tone = 'neutral',
  ariaLabel,
  className,
}: SegmentedControlProps<T>) {
  const btnRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const currentIndex = options.findIndex((o) => o.value === value);

  // Arrow keys move selection *and* focus, per the WAI-ARIA radio-group pattern.
  // A disabled option is STEPPED OVER rather than landed on: the pattern moves
  // selection with focus, so stopping there would either select an unavailable
  // value or strand the caret on a dead segment.
  const move = (toIndex: number, direction: 1 | -1 = 1) => {
    const n = options.length;
    if (n === 0) return;
    for (let hop = 0; hop < n; hop++) {
      const idx = (((toIndex + hop * direction) % n) + n) % n;
      const next = options[idx];
      if (!next || next.disabled) continue;
      if (next.value !== value) onChange(next.value);
      btnRefs.current[idx]?.focus();
      return;
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    switch (e.key) {
      case 'ArrowLeft':
      case 'ArrowUp':
        e.preventDefault();
        move(currentIndex - 1, -1);
        break;
      case 'ArrowRight':
      case 'ArrowDown':
        e.preventDefault();
        move(currentIndex + 1, 1);
        break;
      case 'Home':
        e.preventDefault();
        move(0, 1);
        break;
      case 'End':
        e.preventDefault();
        move(options.length - 1, -1);
        break;
    }
  };

  const isGrid = variant === 'grid';

  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      // A composite widget must be focusable, but the roving tabindex below puts
      // the SELECTED radio in the tab order — so the group itself is reachable
      // programmatically only (`-1`), never as a second tab stop.
      tabIndex={-1}
      onKeyDown={onKeyDown}
      className={cn(
        isGrid
          ? 'grid gap-1.5'
          : 'inline-flex items-center gap-0.5 rounded-lg bg-foreground/[0.04] p-0.5',
        className
      )}
      style={
        isGrid ? { gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` } : undefined
      }
    >
      {options.map((opt, i) => {
        const selected = opt.value === value;
        const Icon = opt.icon;
        return (
          <button
            key={opt.value}
            ref={(el) => {
              btnRefs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-disabled={opt.disabled || undefined}
            tabIndex={selected || (currentIndex === -1 && i === 0) ? 0 : -1}
            title={opt.title}
            onClick={() => {
              if (!opt.disabled) onChange(opt.value);
            }}
            className={cn(
              'inline-flex items-center justify-center gap-1 whitespace-nowrap transition-all',
              opt.disabled && 'cursor-not-allowed opacity-45',
              isGrid
                ? cn(
                    'rounded-lg border py-1.5 text-[11px] font-medium',
                    selected
                      ? 'border-brand/40 bg-brand/10 text-brand-soft'
                      : 'border-foreground/[0.06] bg-foreground/[0.02] text-foreground/45 hover:border-foreground/10 hover:text-foreground/70'
                  )
                : cn(
                    TRACK_ITEM_SIZE[size],
                    selected ? TRACK_ACTIVE[tone] : 'text-foreground/45 hover:text-foreground/70'
                  )
            )}
          >
            {Icon ? (
              <Icon size={isGrid ? ICON_SIZE.sm : ICON_SIZE[size]} aria-hidden="true" />
            ) : null}
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
