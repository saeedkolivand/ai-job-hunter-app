import type { LucideIcon } from 'lucide-react';
import type { ElementType, ReactNode } from 'react';
import { motion } from 'motion/react';
import { cn } from '@/lib/cn';
import { transition } from '@/lib/motion';

interface OptionTileProps {
  icon: LucideIcon | ElementType;
  label: string;
  description?: string;
  selected: boolean;
  onClick: () => void;
  /** Optional extra content below the description */
  children?: ReactNode;
  /** layoutId for the animated selection ring — must be unique per option group */
  layoutId?: string;
}

/**
 * Selectable tile with icon, label, and optional description.
 * Used in settings preference grids (tone, remote preference, performance mode).
 */
export function OptionTile({
  icon: Icon,
  label,
  description,
  selected,
  onClick,
  children,
  layoutId,
}: OptionTileProps) {
  return (
    <motion.button
      type="button"
      onClick={onClick}
      whileHover={{ scale: 1.02 }}
      whileTap={{ scale: 0.98 }}
      className={cn(
        'relative flex flex-col items-center gap-2 rounded-xl border p-4 text-center transition-all duration-150',
        selected
          ? 'border-brand-soft/50 bg-brand-soft/10 glow-subtle'
          : 'border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/10'
      )}
    >
      <div
        className={cn(
          'rounded-full p-2 transition-colors',
          selected ? 'bg-brand-soft/20' : 'bg-white/5'
        )}
      >
        <Icon
          size={20}
          className={cn('transition-colors', selected ? 'text-brand-soft' : 'text-foreground/40')}
        />
      </div>

      <div className="space-y-0.5">
        <div
          className={cn(
            'text-sm font-medium transition-colors',
            selected ? 'text-foreground' : 'text-foreground/70'
          )}
        >
          {label}
        </div>
        {description && <div className="text-xs text-foreground/40">{description}</div>}
      </div>

      {children}

      {selected && layoutId && (
        <motion.div
          layoutId={layoutId}
          className="absolute inset-0 rounded-xl border-2 border-brand-soft/30"
          transition={transition.selection}
        />
      )}
    </motion.button>
  );
}
