import type { ReactNode } from 'react';

import { cn } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface WizardStepProps {
  stepIndex: number;
  totalSteps: number;
  title: string;
  description: string;
  /** `center` vertically centers short steps; `top` lets growing steps flow from the top. */
  align: 'center' | 'top';
  children: ReactNode;
}

/**
 * Reading-column wrapper for every Resume Builder wizard step: a bounded, centered
 * column with a consistent eyebrow/title/description header. Assumes its parent is
 * the scroll container, so `center` alignment can vertically center against the
 * viewport via `min-h-full`.
 */
export function WizardStep({
  stepIndex,
  totalSteps,
  title,
  description,
  align,
  children,
}: WizardStepProps) {
  const { t } = useTranslation();

  return (
    <div
      className={cn(
        'mx-auto w-full max-w-2xl',
        align === 'center' ? 'flex min-h-full flex-col justify-center' : 'pt-2'
      )}
    >
      <div className="mb-5 space-y-1">
        <p className="text-[10px] uppercase tracking-[0.16em] text-foreground/40">
          {t('build.wizard.stepCounter', { current: stepIndex + 1, total: totalSteps })}
        </p>
        <h2 className="text-base font-semibold text-foreground/90">{title}</h2>
        <p className="text-sm text-foreground/50">{description}</p>
      </div>
      {children}
    </div>
  );
}
