import { Loader2 } from 'lucide-react';
import type { CSSProperties, ReactNode } from 'react';

import { cn } from '../../lib/cn';

/** A dot colour: a preset name or any CSS colour string. */
export type TimelineColor = 'brand' | 'blue' | 'green' | 'red' | 'gray' | (string & {});

export interface TimelineItem {
  /** The node's content (right side, or per `mode`). */
  children: ReactNode;
  /** Dot colour — a preset or any CSS colour. Default `brand`. Ignored when `dot` is set. */
  color?: TimelineColor;
  /** Custom node rendered in place of the default ring dot. */
  dot?: ReactNode;
  /** Opposite-side label (e.g. a timestamp). In the default layout it renders muted above the content. */
  label?: ReactNode;
}

export interface TimelineProps {
  /** Ordered nodes (oldest first unless `reverse`). */
  items: TimelineItem[];
  /**
   * Label/content placement. Unset = a compact left rail with the
   * label muted above the content. `left`/`right`/`alternate` put the label on the
   * opposite side of a centered rail.
   */
  mode?: 'left' | 'right' | 'alternate';
  /** Ghost trailing node (e.g. "Waiting…"); shows a spinner dot unless `pendingDot` is given. */
  pending?: ReactNode;
  /** Custom dot for the pending node. Default: a spinning loader. */
  pendingDot?: ReactNode;
  /** Newest-first: reverses the items and moves `pending` to the top. */
  reverse?: boolean;
  className?: string;
}

/** Colour presets; `brand` maps to the app accent, the rest to the shared palette. */
const PRESET_COLORS: Record<string, string> = {
  brand: 'var(--color-brand)',
  blue: '#3b82f6',
  green: '#10b981',
  red: '#ef4444',
  gray: 'color-mix(in srgb, var(--color-foreground) 40%, transparent)',
};

const resolveColor = (color?: TimelineColor): string =>
  color ? (PRESET_COLORS[color] ?? color) : 'var(--color-brand)';

/** A ring dot coloured per item, or a custom node centred on the rail. */
function DotNode({ color, dot }: { color?: TimelineColor; dot?: ReactNode }) {
  if (dot) return <span className="flex size-3 items-center justify-center">{dot}</span>;
  return (
    <span
      className="size-2.5 rounded-full border-2 bg-transparent"
      style={{ borderColor: resolveColor(color) } as CSSProperties}
    />
  );
}

interface Row {
  key: string;
  color?: TimelineColor;
  dot?: ReactNode;
  label?: ReactNode;
  children: ReactNode;
  muted?: boolean;
}

/**
 * Vertical timeline: a connecting rail of coloured
 * (or custom) dots with content to the side. Supports opposite-side labels via
 * `mode` (`left`/`right`/`alternate`), a trailing `pending` node with a spinner,
 * and `reverse` (newest-first). Brand-aware: the default dot colour is the accent.
 */
export function Timeline({
  items,
  mode,
  pending,
  pendingDot,
  reverse = false,
  className,
}: TimelineProps) {
  const ordered = reverse ? [...items].reverse() : items;
  const rows: Row[] = ordered.map((it, i) => ({
    key: `tl-${i}`,
    color: it.color,
    dot: it.dot,
    label: it.label,
    children: it.children,
  }));

  if (pending != null) {
    const pendingRow: Row = {
      key: 'tl-pending',
      dot: pendingDot ?? <Loader2 size={12} className="animate-spin text-foreground/40" />,
      children: pending,
      muted: true,
    };
    if (reverse) rows.unshift(pendingRow);
    else rows.push(pendingRow);
  }

  return (
    <ul role="list" className={cn('m-0 list-none p-0', className)}>
      {rows.map((row, i) => {
        const last = i === rows.length - 1;

        const rail = (
          <div className="flex flex-col items-center self-stretch">
            <span className="flex h-5 items-center">
              <DotNode color={row.color} dot={row.dot} />
            </span>
            {!last && <span className="w-px flex-1 bg-foreground/10" />}
          </div>
        );

        const content = (
          <div
            className={cn(
              'min-w-0 text-[13px] leading-relaxed',
              row.muted ? 'text-foreground/45' : 'text-foreground/80'
            )}
          >
            {row.children}
          </div>
        );

        const label =
          row.label != null ? (
            <div className="text-[11px] leading-relaxed text-foreground/45">{row.label}</div>
          ) : null;

        // Compact default layout — rail on the left, label muted above the content.
        if (mode == null) {
          return (
            <li key={row.key} className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3">
              {rail}
              <div className={cn(!last && 'pb-5')}>
                {label}
                {content}
              </div>
            </li>
          );
        }

        // Opposite-label layout (left / right / alternate) — rail centred.
        const contentRight = mode === 'left' || (mode === 'alternate' && i % 2 === 0);
        const labelCell = (
          <div
            className={cn(
              'flex flex-col',
              contentRight ? 'items-end text-right' : 'items-start text-left'
            )}
          >
            {label}
          </div>
        );
        const contentCell = (
          <div className={cn(!last && 'pb-5', contentRight ? 'text-left' : 'text-right')}>
            {content}
          </div>
        );

        return (
          <li key={row.key} className="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] gap-x-3">
            {contentRight ? labelCell : contentCell}
            {rail}
            {contentRight ? contentCell : labelCell}
          </li>
        );
      })}
    </ul>
  );
}
