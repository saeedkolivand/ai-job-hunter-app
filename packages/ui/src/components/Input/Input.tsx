import { X } from 'lucide-react';
import { forwardRef, type InputHTMLAttributes, type ReactNode, useRef } from 'react';

import { cn } from '../../lib/cn';

export interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'prefix'> {
  variant?: 'default' | 'glass' | 'unstyled';
  /**
   * Icon/element rendered before the input text. When present, a wrapper div
   * owns the border and focus ring — the inner input loses its own chrome.
   */
  prefix?: ReactNode;
  /**
   * Icon/element rendered after the input text. Same wrapper behaviour as prefix.
   */
  suffix?: ReactNode;
  /**
   * Show a clear (×) button while the field holds a value (antd `allowClear`).
   * Clearing dispatches a native input event so the bound `onChange` fires with
   * an empty string — assumes a controlled `value`.
   */
  allowClear?: boolean;
  /** Extra className merged into the wrapper div (only active when prefix or suffix is set). */
  wrapperClassName?: string;
}

function ClearButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      type="button"
      aria-label="Clear"
      tabIndex={-1}
      // Keep focus on the input — blur-before-click would hide the button first.
      onMouseDown={(e) => e.preventDefault()}
      onClick={onClick}
      className="flex shrink-0 items-center justify-center rounded-full p-0.5 text-foreground/30 transition-colors hover:text-foreground/70"
    >
      <X size={12} />
    </button>
  );
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  (
    {
      className,
      variant = 'glass',
      type = 'text',
      prefix,
      suffix,
      allowClear,
      wrapperClassName,
      ...props
    },
    ref
  ) => {
    const unstyled = variant === 'unstyled';
    const innerRef = useRef<HTMLInputElement | null>(null);
    const setRefs = (node: HTMLInputElement | null) => {
      innerRef.current = node;
      if (typeof ref === 'function') ref(node);
      else if (ref) ref.current = node;
    };

    // Clear by setting the native value + dispatching `input`, so React's
    // onChange runs and a controlled parent receives ''.
    const clear = () => {
      const el = innerRef.current;
      if (!el) return;
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
      setter?.call(el, '');
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.focus();
    };

    const showClear =
      allowClear === true && !props.disabled && String(props.value ?? '').length > 0;

    // Wrapped mode: prefix/suffix sit inside a styled container; the inner
    // <input> strips its own border/outline so the ring belongs to the wrapper.
    if (prefix !== undefined || suffix !== undefined) {
      return (
        <div
          className={cn(
            'flex items-center gap-2 rounded-lg px-2.5 transition-shadow duration-150',
            !unstyled &&
              variant !== 'glass' && [
                'border border-[var(--border-clear)] bg-[rgb(var(--glass-rgb)/0.08)] shadow-sm',
              ],
            !unstyled && variant === 'glass' && 'glass shadow-sm',
            // Focus ring on the wrapper, not the input.
            'focus-within:outline-none focus-within:ring-2 focus-within:ring-brand/50 focus-within:ring-offset-1 focus-within:ring-offset-transparent',
            wrapperClassName
          )}
        >
          {prefix !== undefined && <span className="shrink-0 text-foreground/40">{prefix}</span>}
          <input
            ref={setRefs}
            type={type}
            className={cn(
              'min-w-0 flex-1 bg-transparent text-sm text-foreground placeholder:text-foreground/30',
              'border-none p-0 focus:outline-none',
              className
            )}
            // Tailwind v4 unlayered :focus-visible beats layered focus: classes →
            // inline style is the only reliable override for the default UA ring.
            style={{ outline: 'none' }}
            {...props}
          />
          {showClear && <ClearButton onClick={clear} />}
          {suffix !== undefined && <span className="shrink-0 text-foreground/40">{suffix}</span>}
        </div>
      );
    }

    // Bare mode.
    const input = (
      <input
        ref={setRefs}
        type={type}
        className={cn(
          !unstyled &&
            'input-field rounded-lg px-3 py-2 text-sm text-foreground placeholder:text-foreground/30 transition-shadow duration-150',
          variant === 'default' && 'bg-white/5 shadow-sm',
          variant === 'glass' && 'glass shadow-sm',
          // Reserve room so long text doesn't slide under the clear button.
          allowClear && 'pr-9',
          className
        )}
        {...props}
      />
    );

    if (!allowClear) return input;

    // Overlay the clear button on the right so the input keeps its own chrome
    // and padding (assumes a full-width input — the common case).
    return (
      <div className="relative w-full">
        {input}
        {showClear && (
          <span className="absolute right-2.5 top-1/2 -translate-y-1/2">
            <ClearButton onClick={clear} />
          </span>
        )}
      </div>
    );
  }
);

Input.displayName = 'Input';
