import type { ReactNode } from 'react';

import { cn } from '@ajh/ui';

interface WizardStepProps {
  title: string;
  description: string;
  /** `center` vertically centers short steps; `top` lets growing steps flow from the top. */
  align: 'center' | 'top';
  children: ReactNode;
}

/**
 * Reading-column wrapper for every Resume Builder wizard step: a bounded, centered
 * column with a consistent title/description header (the step counter lives in the wizard's top bar). Assumes its parent is
 * the scroll container, so `center` alignment can vertically center against the
 * viewport via `min-h-full`.
 */
export function WizardStep({ title, description, align, children }: WizardStepProps) {
  return (
    <div
      className={cn(
        'mx-auto w-full max-w-2xl',
        align === 'center' ? 'flex min-h-full flex-col justify-center' : 'pt-2'
      )}
    >
      <div className="surface-card p-6 shadow-sm sm:p-8">
        <div className="mb-5 space-y-1">
          <h2 className="text-base font-semibold text-foreground/90">{title}</h2>
          <p className="text-sm text-foreground/50">{description}</p>
        </div>
        {children}
      </div>
    </div>
  );
}
