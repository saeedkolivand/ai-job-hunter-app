import type { HTMLAttributes } from 'react';

import { cn } from '../../lib/cn';

export type OptionalHintProps = HTMLAttributes<HTMLSpanElement>;

/**
 * Inline "optional" marker (#19). Italic, muted, caption-sized — placed next to
 * (or just below) the element it qualifies so optional fields read as optional
 * at a glance. Defaults to the word "optional"; pass children to localise or
 * customise (e.g. "optional — appears on your résumé header").
 */
export function OptionalHint({ className, children, ...rest }: OptionalHintProps) {
  return (
    <span className={cn('text-caption italic text-foreground/50', className)} {...rest}>
      {children ?? 'optional'}
    </span>
  );
}
