import { X } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

export interface HandEditNudgeProps {
  className?: string;
}

/**
 * Small, dismissible hint shown after a generation completes: suggests
 * hand-editing a line or two in the candidate's own voice — honest and short,
 * never claims to detect or "fix" AI-sounding text (that's the quality
 * report's job). Dismissing hides it for this mount only; the host keys this
 * component by something that changes each generation (e.g. the quality
 * report's `generatedAt`) so a NEW generation remounts it — "once per
 * generation, not per render" is a remount contract, not internal state here.
 */
export function HandEditNudge({ className }: HandEditNudgeProps) {
  const { t } = useTranslation();
  const [dismissed, setDismissed] = useState(false);

  if (dismissed) return null;

  return (
    <div
      role="status"
      className={cn(
        'flex shrink-0 items-center justify-between gap-2 rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-2 text-[11px] text-foreground/55',
        className
      )}
    >
      <span>{t('quality.nudge.message')}</span>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={() => setDismissed(true)}
        aria-label={t('quality.nudge.dismiss')}
        className="h-6 w-6 shrink-0 p-0"
      >
        <X size={11} />
      </Button>
    </div>
  );
}
