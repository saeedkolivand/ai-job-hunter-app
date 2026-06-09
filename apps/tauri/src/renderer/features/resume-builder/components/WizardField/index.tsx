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
  /** Inline validation message rendered below the control; also flags the control invalid. */
  error?: string;
  children: ReactNode;
}

/**
 * Labeled field wrapper for Resume Builder wizard steps. Wires the `<label>` to its
 * single control automatically: it reuses an explicit `htmlFor`, then the child's
 * own `id`, otherwise it generates an id and injects it into the child — so callers
 * never have to repeat an id on both the label and the control. When `error` is set
 * it renders inline error text and marks the control `aria-invalid`.
 */
export function WizardField({ label, hint, htmlFor, error, children }: WizardFieldProps) {
  const generatedId = useId();
  const child = Children.only(children);
  const isElement = isValidElement(child);
  const childProps = isElement ? (child.props as { id?: string }) : null;
  const childId = typeof childProps?.id === 'string' ? childProps.id : undefined;
  const labelFor = htmlFor ?? childId ?? generatedId;

  // Inject the generated id (when none exists) and the invalid flag (when errored)
  // into the single control, preserving any explicit id the caller already set.
  const injected: { id?: string; 'aria-invalid'?: boolean } = {};
  if (isElement && !childId && !htmlFor) injected.id = generatedId;
  if (isElement && error) injected['aria-invalid'] = true;

  const control =
    isElement && Object.keys(injected).length > 0
      ? cloneElement(child as ReactElement<Record<string, unknown>>, injected)
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
      {error && <p className="text-[10px] text-red-400/80">{error}</p>}
    </div>
  );
}
