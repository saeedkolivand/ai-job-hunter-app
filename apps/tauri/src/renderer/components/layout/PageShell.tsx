import type { ReactNode } from 'react';

import { cn } from '@/lib/cn';

import { PageHeader } from './PageHeader';
import { PageTransition } from './PageTransition';

interface PageShellProps {
  /** Page title — passed to PageHeader */
  title: string;
  subtitle?: string;
  badge?: string;
  /** Slot for header action buttons */
  actions?: ReactNode;
  children: ReactNode;
  /** Override the default scrollable content class */
  contentClassName?: string;
  /** Max-width of the inner content column — default max-w-6xl */
  maxWidth?: string;
}

/**
 * Standard page wrapper.
 * Provides: page transition, header, consistent padding, scrollable content.
 *
 * Usage:
 *   <PageShell title="Jobs" subtitle="Browse and manage applications">
 *     <YourContent />
 *   </PageShell>
 */
export function PageShell({
  title,
  subtitle,
  badge,
  actions,
  children,
  contentClassName,
  maxWidth = 'max-w-6xl',
}: PageShellProps) {
  return (
    <PageTransition className="flex h-full flex-col overflow-hidden">
      <div className={cn('flex-1 overflow-y-auto px-10 py-10', contentClassName)}>
        <div className={cn('mx-auto', maxWidth)}>
          <PageHeader title={title} subtitle={subtitle} badge={badge} actions={actions} />
          {children}
        </div>
      </div>
    </PageTransition>
  );
}
