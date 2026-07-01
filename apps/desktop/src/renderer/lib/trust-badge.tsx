import { ShieldAlert } from 'lucide-react';

import type { JobTrustAssessment } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, cn, HoverPopover, Tag } from '@ajh/ui';

/**
 * Ghost-job trust badge — flag-only, V1: renders NOTHING for `level === 'high'`
 * or a missing `trust` (no badge = trusted, keeps the common case noise-free).
 * The visible label names the signal explicitly ("Low trust" / "Medium trust")
 * so it can't be mistaken for a generic "unverified account" state and reads
 * distinctly from the adjacent match-score Low/Medium/High tag. `low` gets the
 * stronger (error) tint, `medium` the softer (warning) one — both stay advisory,
 * never alarming.
 *
 * The "why" (which flags fired) is reachable by mouse AND keyboard via a
 * `HoverPopover` (focus-triggerable, unlike a native `title`), plus an
 * always-present sr-only suffix folded into the trigger's accessible name so
 * screen readers get the detail without needing to open the popover.
 */
export function TrustBadge({
  trust,
  className,
  strong,
  interactive = true,
}: {
  trust?: JobTrustAssessment;
  className?: string;
  /** Force an opaque solid fill instead of the default translucent tint — use
   *  when the badge sits over a tinted/gradient surface (e.g. a selected row
   *  highlight) where the translucent tint's contrast against that surface
   *  isn't guaranteed. */
  strong?: boolean;
  /** Set false on a surface where rows are deliberately never real tab stops
   *  (an aria-activedescendant listbox) — a focusable popover trigger there
   *  would add an unexpected extra stop per row. Renders the badge + an
   *  sr-only reasons suffix instead, read as part of the row's own content
   *  when it becomes the active descendant. Default true. */
  interactive?: boolean;
}) {
  const { t } = useTranslation();
  if (!trust || trust.level === 'high') return null;

  const isLow = trust.level === 'low';
  const levelLabel = t(`jobs.trust.level.${trust.level}`);
  const reasons = trust.flags.map((flag) => t(`jobs.trust.flags.${flag}`)).join(', ');

  const badge = (
    <Tag
      color={isLow ? 'error' : 'warning'}
      icon={<ShieldAlert size={8} aria-hidden="true" />}
      className={cn(
        'rounded-full px-1.5 py-0.5 uppercase tracking-wider',
        strong &&
          (isLow
            ? 'border-transparent bg-red-700 text-white'
            : 'border-transparent bg-amber-700 text-white'),
        className
      )}
    >
      {levelLabel}
    </Tag>
  );

  if (!interactive || !reasons) {
    return (
      <span className="inline-flex items-center gap-1">
        {badge}
        {reasons && <span className="sr-only">: {reasons}</span>}
      </span>
    );
  }

  return (
    <HoverPopover
      placement="top"
      ariaLabel={reasons}
      trigger={
        <Button variant="unstyled" className="inline-flex items-center rounded-full">
          {badge}
          <span className="sr-only">: {reasons}</span>
        </Button>
      }
    >
      <p className="dropdown-surface max-w-[220px] px-3 py-2 text-fine-print leading-snug text-foreground/70">
        {reasons}
      </p>
    </HoverPopover>
  );
}
