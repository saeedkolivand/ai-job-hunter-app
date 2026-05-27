import { forwardRef, type InputHTMLAttributes } from 'react';

import { cn } from '../../lib/cn';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  variant?: 'default' | 'glass';
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, variant = 'glass', type = 'text', ...props }, ref) => {
    return (
      <input
        ref={ref}
        type={type}
        className={cn(
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
