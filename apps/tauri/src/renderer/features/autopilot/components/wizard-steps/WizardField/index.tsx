import type { ReactNode } from 'react';

interface WizardFieldProps {
  label: string;
  hint?: string;
  /** Optional inline element rendered next to the label (e.g. a status badge). */
  badge?: ReactNode;
  children: ReactNode;
}

export function WizardField({ label, hint, badge, children }: WizardFieldProps) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-1.5">
        <label className="text-xs font-medium text-foreground/60">{label}</label>
        {badge}
        {hint && <span className="text-[10px] text-foreground/30">{hint}</span>}
      </div>
      {children}
    </div>
  );
}
