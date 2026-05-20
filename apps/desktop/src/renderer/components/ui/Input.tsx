import { type InputHTMLAttributes, forwardRef } from 'react';
import { cn } from '@/lib/cn';

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
          'appearance-none [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none',
          variant === 'default' && 'bg-white/5 border border-white/[0.06]',
          variant === 'glass' &&
            'border border-white/[0.06] bg-[rgba(15,15,25,0.45)] hover:border-white/10 hover:bg-[rgba(15,15,25,0.55)] focus:border-brand/35 focus:bg-[rgba(15,15,25,0.55)]',
          className
        )}
        {...props}
      />
    );
  }
);

Input.displayName = 'Input';
