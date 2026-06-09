import {
  Children,
  cloneElement,
  isValidElement,
  type ReactElement,
  type ReactNode,
  useId,
} from 'react';

interface WizardFieldProps {
  label: string;
  hint?: string;
  /** Associates the label with a control by id. Overrides the child's own id when set. */
  htmlFor?: string;
  children: ReactNode;
}

/**
 * Labeled field wrapper for Resume Builder wizard steps. Wires the `<label>` to its
 * single control automatically: it reuses an explicit `htmlFor`, then the child's
 * own `id`, otherwise it generates an id and injects it into the child — so callers
 * never have to repeat an id on both the label and the control.
 */
export function WizardField({ label, hint, htmlFor, children }: WizardFieldProps) {
  const generatedId = useId();
  const child = Children.only(children);
  const childProps = isValidElement(child) ? (child.props as { id?: string }) : null;
  const childId = typeof childProps?.id === 'string' ? childProps.id : undefined;
  const labelFor = htmlFor ?? childId ?? generatedId;

  // Inject the generated id only when the child has no id and none was provided —
  // preserving any explicit id the caller already set on the control.
  const control =
    isValidElement(child) && !childId && !htmlFor
      ? cloneElement(child as ReactElement<{ id?: string }>, { id: generatedId })
      : child;

  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-1.5">
        <label htmlFor={labelFor} className="text-xs font-medium text-foreground/60">
          {label}
        </label>
        {hint && <span className="text-[10px] text-foreground/30">{hint}</span>}
      </div>
      {control}
    </div>
  );
}
