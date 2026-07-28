import { cloneElement, isValidElement, type ReactNode } from 'react';

interface WizardFieldProps {
  label: string;
  hint?: string;
  /** Optional inline element rendered next to the label (e.g. a status badge). */
  badge?: ReactNode;
  /** Associates the label with a control by id (for inputs that aren't nested children). */
  htmlFor?: string;
  /** Inline validation message rendered below the control. The bound control sets its own `aria-invalid`. */
  error?: string;
  children: ReactNode;
}

/**
 * Label + optional hint/error around one wizard control.
 *
 * A11y: the hint and error are announced as the control's DESCRIPTION rather than
 * left as orphan text. Some hints are load-bearing — the page-budget field's hint
 * is the only explanation of why the control is disabled — and a bare `<span>`
 * next to a label reaches no screen-reader user. So when `htmlFor` is given (the
 * signal that `children` is one labelled control), both get stable ids and are
 * wired onto the child via `aria-describedby`. Description, not name: it stays out
 * of the accessible NAME, matching `@ajh/ui`'s `Switch`.
 *
 * The child is cloned rather than asked to opt in, so every current and future
 * usage benefits without touching call sites. Only a single element child can
 * carry the attribute; a fragment/wrapper child silently keeps the old behaviour.
 */
export function WizardField({ label, hint, badge, htmlFor, error, children }: WizardFieldProps) {
  const hintId = htmlFor && hint ? `${htmlFor}-hint` : undefined;
  const errorId = htmlFor && error ? `${htmlFor}-error` : undefined;
  const describedBy = [hintId, errorId].filter(Boolean).join(' ') || undefined;

  const control =
    describedBy && isValidElement<{ 'aria-describedby'?: string }>(children)
      ? cloneElement(children, { 'aria-describedby': describedBy })
      : children;

  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-1.5">
        <label htmlFor={htmlFor} className="text-xs font-medium text-foreground/60">
          {label}
        </label>
        {badge}
        {hint && (
          <span id={hintId} className="text-[10px] text-foreground/30">
            {hint}
          </span>
        )}
      </div>
      {control}
      {error && (
        <p id={errorId} className="text-[10px] text-red-400/80">
          {error}
        </p>
      )}
    </div>
  );
}
