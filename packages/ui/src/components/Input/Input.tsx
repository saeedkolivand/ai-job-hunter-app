import { forwardRef, type InputHTMLAttributes } from 'react';

import { cn } from '../../lib/cn';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  variant?: 'default' | 'glass' | 'unstyled';
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, variant = 'glass', type = 'text', ...props }, ref) => {
    // `unstyled` is an escape hatch for inline/embedded fields (e.g. a borderless
    // input sitting inside a card row) that supply their own appearance via
    // className. It still routes through this primitive for consistency.
    const unstyled = variant === 'unstyled';
    return (
      <input
        ref={ref}
        type={type}
        className={cn(
          !unstyled &&
            'input-field rounded-lg px-3 py-2 text-sm text-foreground placeholder:text-foreground/30',
          variant === 'default' && 'bg-white/5',
          variant === 'glass' && 'glass-dropdown',
          className
        )}
        {...props}
      />
    );
  }
);

Input.displayName = 'Input';
