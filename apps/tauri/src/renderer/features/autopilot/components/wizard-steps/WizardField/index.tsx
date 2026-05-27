import type { ReactNode } from 'react';

interface WizardFieldProps {
  label: string;
  hint?: string;
  children: ReactNode;
}

export function WizardField({ label, hint, children }: WizardFieldProps) {
  return (
    <div className="space-y-1.5">
      <div>
        <label className="text-xs font-medium text-foreground/60">{label}</label>
        {hint && <span className="ml-1.5 text-[10px] text-foreground/30">{hint}</span>}
      </div>
      {children}
    </div>
  );
}
