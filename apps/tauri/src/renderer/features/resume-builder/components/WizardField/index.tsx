import type { ReactNode } from 'react';

interface WizardFieldProps {
  label: string;
  hint?: string;
  /** Associates the label with a control by id (for inputs that aren't nested children). */
  htmlFor?: string;
  children: ReactNode;
}

/** Labeled field wrapper for Resume Builder wizard steps. */
export function WizardField({ label, hint, htmlFor, children }: WizardFieldProps) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-1.5">
        <label htmlFor={htmlFor} className="text-xs font-medium text-foreground/60">
          {label}
        </label>
        {hint && <span className="text-[10px] text-foreground/30">{hint}</span>}
      </div>
      {children}
    </div>
  );
}
