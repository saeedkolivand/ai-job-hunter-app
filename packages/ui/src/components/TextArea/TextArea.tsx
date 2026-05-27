import type { TextareaHTMLAttributes } from 'react';

import { cn } from '../lib/cn';

export interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  variant?: 'default' | 'glass';
}

export function TextArea({ className, variant = 'default', ...props }: TextAreaProps) {
  return (
    <textarea
      className={cn(
        'input-field w-full resize-none text-sm leading-relaxed text-foreground placeholder:text-foreground/30',
        variant === 'glass' && 'glass-dropdown rounded-lg px-3 py-2',
        variant === 'default' && 'bg-transparent',
        className
      )}
      {...props}
    />
  );
}
