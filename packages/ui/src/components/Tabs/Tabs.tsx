import type { LucideIcon, LucideProps } from 'lucide-react';
import { type KeyboardEvent, type ReactNode, useRef } from 'react';

import { cn } from '../../lib/cn';

export interface TabItem<T extends string> {
  value: T;
  label: ReactNode;
  /** Optional leading icon. */
  icon?: LucideIcon;
  /** Explicit tab element id (for aria-controls wiring by consumers). */
  id?: string;
  /** Id of the tabpanel this tab controls (APG aria-controls). */
  ariaControls?: string;
}

export interface TabsProps<T extends string> {
  items: readonly TabItem<T>[];
  value: T;
  onChange: (value: T) => void;
  /** Accessible name for the tablist. */
  ariaLabel: string;
  /** Visual density. Default `'sm'`. */
  size?: 'sm' | 'md';
  /**
   * Id prefix. Tab buttons get id `${idBase}-${item.value}` when `item.id`
   * is not explicitly set, so consumers can wire `aria-controls` to panels.
   */
  idBase?: string;
  className?: string;
}

const SIZE_CLS = {
  sm: 'text-[11px]',
  md: 'text-xs',
} as const;

const ICON_SIZE: Record<'sm' | 'md', LucideProps['size']> = { sm: 12, md: 13 };

/**
 * ARIA-compliant tab bar with roving tabindex + full keyboard navigation
 * (ArrowLeft/Right/Up/Down, Home, End). Styling uses design tokens only —
 * no hardcoded colours.
 */
export function Tabs<T extends string>({
  items,
  value,
  onChange,
  ariaLabel,
  size = 'sm',
  idBase,
  className,
}: TabsProps<T>) {
  const btnRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const focusedIndex = useRef<number>(0);
  const currentIndex = items.findIndex((item) => item.value === value);

  const move = (toIndex: number) => {
    const n = items.length;
    if (n === 0) return;
    const idx = ((toIndex % n) + n) % n;
    const next = items[idx];
    if (!next) return;
    if (next.value !== value) onChange(next.value);
    btnRefs.current[idx]?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const base = currentIndex >= 0 ? currentIndex : focusedIndex.current;
    switch (e.key) {
      case 'ArrowLeft':
      case 'ArrowUp':
        e.preventDefault();
        move(base - 1);
        break;
      case 'ArrowRight':
      case 'ArrowDown':
        e.preventDefault();
        move(base + 1);
        break;
      case 'Home':
        e.preventDefault();
        move(0);
        break;
      case 'End':
        e.preventDefault();
        move(items.length - 1);
        break;
    }
  };

  return (
    <div
      role="tablist"
      aria-label={ariaLabel}
      onKeyDown={onKeyDown}
      className={cn(
        'flex items-center gap-1 overflow-x-auto border-b border-[var(--border-mid)]',
        className
      )}
    >
      {items.map((item, i) => {
        const selected = item.value === value;
        const Icon = item.icon;
        const tabId = item.id ?? (idBase ? `${idBase}-${item.value}` : undefined);

        return (
          <button
            key={item.value}
            ref={(el) => {
              btnRefs.current[i] = el;
            }}
            type="button"
            role="tab"
            id={tabId}
            aria-selected={selected}
            aria-controls={item.ariaControls}
            tabIndex={selected || (currentIndex === -1 && i === 0) ? 0 : -1}
            onFocus={() => {
              focusedIndex.current = i;
            }}
            onClick={() => onChange(item.value)}
            className={cn(
              'inline-flex min-h-[24px] items-center gap-1 rounded px-2.5 py-1 font-medium transition-colors whitespace-nowrap',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1 focus-visible:ring-offset-transparent',
              SIZE_CLS[size],
              selected
                ? 'bg-brand/15 font-semibold text-foreground'
                : 'text-foreground/70 hover:text-foreground/85'
            )}
          >
            {Icon ? <Icon size={ICON_SIZE[size]} aria-hidden="true" /> : null}
            {item.label}
          </button>
        );
      })}
    </div>
  );
}
