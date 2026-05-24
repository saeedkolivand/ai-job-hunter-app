import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from '../lib/cn';
import { GlassCard } from './GlassCard';

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
  return (
    <GlassCard
      className={cn(
        'group cursor-pointer transition-all duration-150 ease-out',
        active && 'ring-1 ring-brand/40',
        className
      )}
      onClick={onClick}
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
