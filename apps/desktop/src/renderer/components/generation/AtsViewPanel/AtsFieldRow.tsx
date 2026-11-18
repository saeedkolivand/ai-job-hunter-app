import { AlertTriangle, CheckCircle2, CircleSlash } from 'lucide-react';

import type { AtsField, AtsStatus } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { cn } from '@ajh/ui';

const STATUS_ICON = {
  found: { Icon: CheckCircle2, className: 'text-emerald-400' },
  suspicious: { Icon: AlertTriangle, className: 'text-amber-400' },
  missing: { Icon: CircleSlash, className: 'text-foreground/40' },
} as const satisfies Record<AtsStatus, { Icon: unknown; className: string }>;

interface AtsFieldRowProps {
  label: string;
  field: AtsField;
  /** Shown instead of `field.value` (e.g. a section's heading). */
  value?: string;
  /** Shown for a `found` field that carries no value (e.g. the reading order). */
  foundText?: string;
  /** Replaces the generic reason text of a `missing` field. */
  missingText?: string;
}

/** One parsed field: status icon, what the parser read, and why it is doubtful. */
export function AtsFieldRow({ label, field, value, foundText, missingText }: AtsFieldRowProps) {
  const { t } = useTranslation();
  const { Icon, className } = STATUS_ICON[field.status];
  const shown =
    value ??
    (field.value === 'present' ? t('atsView.present') : field.value) ??
    (field.status === 'found' ? foundText : undefined);
  const reason =
    field.status === 'missing' && missingText
      ? missingText
      : field.reason && t(`atsView.reason.${field.reason}`);
  return (
    <li className="flex items-start gap-2 py-1 text-[12px]">
      <Icon size={13} aria-hidden="true" className={cn('mt-0.5 shrink-0', className)} />
      <span className="sr-only">{t(`atsView.status.${field.status}`)}:</span>
      <span className="w-28 shrink-0 text-foreground/65">{label}</span>
      <span className="min-w-0 flex-1">
        {shown && <span className="break-words text-foreground/80">{shown}</span>}
        {reason && <span className="block text-[11px] text-foreground/65">{reason}</span>}
      </span>
    </li>
  );
}
