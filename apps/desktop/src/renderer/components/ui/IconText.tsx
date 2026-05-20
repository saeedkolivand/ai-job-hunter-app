import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

interface IconTextProps {
  icon: ReactNode;
  children: ReactNode;
  gap?: 'sm' | 'md' | 'lg';
  className?: string;
}

export function IconText({ icon, children, gap = 'md', className }: IconTextProps) {
  return (
    <span
      className={cn(
        'flex items-center',
        {
          'gap-1': gap === 'sm',
          'gap-1.5': gap === 'md',
          'gap-2': gap === 'lg',
        },
        className
      )}
    >
      {icon}
      {children}
    </span>
  );
}
