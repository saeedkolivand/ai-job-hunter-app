import type { LucideIcon } from 'lucide-react';
import type { ElementType } from 'react';

import { cn } from '../lib/cn';

interface SectionLabelProps {
  icon?: LucideIcon | ElementType;
  children: React.ReactNode;
  className?: string;
}

/**
 * Small all-caps label used as card section headers.
 * Consistent tracking, size, and opacity across every card.
 */
export function SectionLabel({ icon: Icon, children, className }: SectionLabelProps) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 text-xs font-semibold uppercase tracking-widest text-foreground/40',
        className
      )}
    >
      {Icon && <Icon size={14} />}
      {children}
    </div>
  );
}
