import type { LucideIcon } from 'lucide-react';
import type { KeyboardEvent, ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { GlassCard } from '../GlassCard';

interface ActionTileProps {
  icon: LucideIcon;
  label: string;
  description?: string;
  badge?: ReactNode;
  onClick?: () => void;
  active?: boolean;
  className?: string;
}

/**
 * Clickable tile with icon, label, optional description and badge.
 * Used in quick-action grids, feature selection, and option pickers.
 *
 * The surface is a `div`, so when it is clickable it carries the button role,
 * tab stop, focus ring and Enter/Space handling a `<button>` would give for
 * free — without them the quick-action grids were mouse-only. A tile with no
 * `onClick` stays inert: no role, no tab stop, nothing for a keyboard user to
 * land on.
 */
export function ActionTile({
  icon: Icon,
  label,
  description,
  badge,
  onClick,
  active = false,
  className,
}: ActionTileProps) {
  const handleKeyDown =
    onClick &&
    ((event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key !== 'Enter' && event.key !== ' ') return;
      // Space scrolls the page on a focused non-button element; Enter doesn't.
      // Before the repeat guard below, deliberately: a HELD space must not
      // start scrolling either, even though it activates only once.
      if (event.key === ' ') event.preventDefault();
      // Auto-repeat: holding the key fires this handler at the OS repeat rate,
      // which on a tile that navigates means a burst of activations from a
      // single press. One press, one `onClick`.
      if (event.repeat) return;
      onClick();
    });

  return (
    <GlassCard
      className={cn(
        'group cursor-pointer transition-all duration-150 ease-out',
        onClick &&
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1 focus-visible:ring-offset-transparent',
        active && 'ring-1 ring-brand/40',
        className
      )}
      onClick={onClick}
      onKeyDown={handleKeyDown}
      role={onClick ? 'button' : undefined}
      tabIndex={onClick ? 0 : undefined}
    >
      <div className="mb-3 flex items-start justify-between">
        <Icon
          size={20}
          className={cn(
            'transition-all duration-150 ease-out group-hover:rotate-3',
            active ? 'text-brand-soft' : 'text-foreground/50 group-hover:text-brand-soft'
          )}
        />
        {badge}
      </div>
      <div className="text-sm font-medium text-foreground/80 transition-colors duration-150 ease-out group-hover:text-foreground">
        {label}
      </div>
      {description && <div className="mt-0.5 text-xs text-foreground/40">{description}</div>}
    </GlassCard>
  );
}
