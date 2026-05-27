import type { LucideIcon } from 'lucide-react';
import type { ElementType } from 'react';

import { cn } from '../../lib/cn';
import { IconBadge } from '../IconBadge';

interface SectionHeaderProps {
  icon: LucideIcon | ElementType;
  title: string;
  description?: string;
  size?: 'sm' | 'md';
  className?: string;
}

/**
 * Titled section header with branded icon badge, title, and optional description.
 * Used at the top of settings panels and feature sections.
 */
export function SectionHeader({
  icon,
  title,
  description,
  size = 'md',
  className,
}: SectionHeaderProps) {
  return (
    <div className={cn('flex items-center gap-3', className)}>
      <IconBadge icon={icon} size={size === 'sm' ? 'sm' : 'md'} />
      <div>
        <div
          className={cn(
            'font-semibold text-foreground/90',
            size === 'sm' ? 'text-sm' : 'text-base'
          )}
        >
          {title}
        </div>
        {description && <div className="text-[11px] text-foreground/40">{description}</div>}
      </div>
    </div>
  );
}
