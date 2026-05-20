import { type ButtonHTMLAttributes, forwardRef } from 'react';
import { Loader2 } from 'lucide-react';
import { cn } from '@/lib/cn';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'glass' | 'ghost' | 'danger' | 'warning' | 'info' | 'success';
  size?: 'sm' | 'md' | 'lg';
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = 'default', size = 'md', loading, disabled, children, ...props }, ref) => {
    return (
      <button
        ref={ref}
        disabled={disabled || loading}
        className={cn(
          // Base
          'inline-flex items-center gap-2 rounded-lg font-medium transition-all duration-150',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1 focus-visible:ring-offset-transparent',
          'active:scale-[0.97] disabled:pointer-events-none disabled:opacity-45',

          // Variants
          variant === 'ghost' && [
            'border border-white/[0.06] bg-transparent text-foreground/55',
            'hover:border-white/10 hover:bg-white/[0.05] hover:text-foreground/80',
          ],
          variant === 'glass' && [
            'border border-white/10 bg-white/[0.07] text-foreground/90 shadow-sm',
            'hover:border-white/15 hover:bg-white/[0.10] hover:text-foreground',
          ],
          variant === 'default' && [
            'border border-white/[0.06] bg-white/[0.04] text-foreground/70',
            'hover:border-white/10 hover:bg-white/[0.08] hover:text-foreground/90',
          ],
          variant === 'danger' && [
            'border border-red-500/25 bg-red-500/10 text-red-300',
            'hover:border-red-500/40 hover:bg-red-500/20 hover:text-red-200',
          ],
          variant === 'warning' && [
            'border border-amber-500/25 bg-amber-500/10 text-amber-300',
            'hover:border-amber-500/40 hover:bg-amber-500/20 hover:text-amber-200',
          ],
          variant === 'info' && [
            'border border-blue-500/25 bg-blue-500/10 text-blue-300',
            'hover:border-blue-500/40 hover:bg-blue-500/20 hover:text-blue-200',
          ],
          variant === 'success' && [
            'border border-emerald-500/25 bg-emerald-500/10 text-emerald-300',
            'hover:border-emerald-500/40 hover:bg-emerald-500/20 hover:text-emerald-200',
          ],

          // Sizes
          size === 'sm' && 'h-7 px-2.5 text-xs',
          size === 'md' && 'h-8 px-3.5 text-sm',
          size === 'lg' && 'h-10 px-5 text-sm',

          className
        )}
        {...props}
      >
        {loading && <Loader2 size={size === 'lg' ? 14 : 12} className="animate-spin" />}
        {children}
      </button>
    );
  }
);

Button.displayName = 'Button';
