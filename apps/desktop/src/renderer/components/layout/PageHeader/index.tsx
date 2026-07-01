import type { ReactNode } from 'react';

import { cn } from '@ajh/ui';

interface PageHeaderProps {
  title: string;
  subtitle?: string;
  badge?: string;
  actions?: ReactNode;
  className?: string;
}

export function PageHeader({ title, subtitle, badge, actions, className }: PageHeaderProps) {
  return (
    <div className={cn('mb-8', className)}>
      <div className="flex items-start justify-between gap-4">
        <div className="flex-1">
          {badge && (
            <div className="mb-2 inline-flex items-center rounded-full bg-brand-soft/10 px-3 py-1 text-xs font-medium text-brand-soft">
              {badge}
            </div>
          )}
          <h1 className="text-gradient text-3xl font-bold tracking-tight">{title}</h1>
          {subtitle && <p className="mt-2 max-w-2xl text-sm text-foreground/55">{subtitle}</p>}
        </div>
        {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
      </div>
    </div>
  );
}
