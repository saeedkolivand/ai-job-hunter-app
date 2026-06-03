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
  neutral: 'bg-white/10 text-foreground/90',
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
  const move = (toIndex: number) => {
    const n = options.length;
    if (n === 0) return;
    const idx = ((toIndex % n) + n) % n;
    const next = options[idx];
    if (!next) return;
    if (next.value !== value) onChange(next.value);
    btnRefs.current[idx]?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    switch (e.key) {
      case 'ArrowLeft':
      case 'ArrowUp':
        e.preventDefault();
        move(currentIndex - 1);
        break;
      case 'ArrowRight':
      case 'ArrowDown':
        e.preventDefault();
        move(currentIndex + 1);
        break;
      case 'Home':
        e.preventDefault();
        move(0);
        break;
      case 'End':
        e.preventDefault();
        move(options.length - 1);
        break;
    }
  };

  const isGrid = variant === 'grid';

  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      onKeyDown={onKeyDown}
      className={cn(
        isGrid
          ? 'grid gap-1.5'
          : 'inline-flex items-center gap-0.5 rounded-lg bg-white/[0.04] p-0.5',
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
            tabIndex={selected || (currentIndex === -1 && i === 0) ? 0 : -1}
            title={opt.title}
            onClick={() => onChange(opt.value)}
            className={cn(
              'inline-flex items-center justify-center gap-1 whitespace-nowrap transition-all',
              isGrid
                ? cn(
                    'rounded-lg border py-1.5 text-[11px] font-medium',
                    selected
                      ? 'border-brand/40 bg-brand/10 text-brand-soft'
                      : 'border-white/[0.06] bg-white/[0.02] text-foreground/45 hover:border-white/10 hover:text-foreground/70'
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
