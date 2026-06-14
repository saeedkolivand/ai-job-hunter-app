import { Loader2 } from 'lucide-react';
import { type ButtonHTMLAttributes, forwardRef } from 'react';

import { cn } from '../../lib/cn';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /**
   * Apple-grammar variants:
   * - `primary` — the signature two-tone accent CTA (action-primary token, rounded utility radius).
   * - `run` / `edit` / `delete` — solid colorful action pills (semantic colour,
   *   the deliberate divergence from Apple's single-accent rule; tokens only).
   * - `default` / `glass` / `ghost` — neutral utility buttons (rounded, not pill).
   * - `danger` / `warning` / `info` / `success` — translucent inline state chips.
   * - `unstyled` — escape hatch; call site owns the look.
   */
  variant?:
    | 'primary'
    | 'default'
    | 'glass'
    | 'ghost'
    | 'run'
    | 'edit'
    | 'delete'
    | 'danger'
    | 'warning'
    | 'info'
    | 'success'
    | 'unstyled';
  size?: 'sm' | 'md' | 'lg';
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = 'default', size = 'md', loading, disabled, children, ...props }, ref) => {
    // `unstyled` is an escape hatch for custom interactive surfaces (clickable
    // cards, segmented controls, icon toggles, inline text links) that supply
    // their own appearance via className. It still routes through this primitive
    // for consistent focus-visible + disabled handling, but injects no chrome.
    const unstyled = variant === 'unstyled';
    // Semantic action pills (run/edit/delete) take the Apple pill radius;
    // `primary` and every neutral utility button stay on the rounded (≈8px)
    // utility radius so the primary CTA matches buttons like `default`.
    const isPill = variant === 'run' || variant === 'edit' || variant === 'delete';
    return (
      <button
        ref={ref}
        disabled={disabled || loading}
        className={cn(
          // Base (skipped for `unstyled` — the call site owns layout/look).
          // Apple: weight 400 (no 500), scale(0.95) press, pill for filled actions.
          !unstyled && [
            'inline-flex items-center justify-center gap-2 font-normal transition-all duration-150',
            isPill ? 'rounded-full' : 'rounded-lg',
            'active:scale-[0.95]',
          ],
          // Accessibility essentials apply to every variant
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1 focus-visible:ring-offset-transparent',
          'disabled:pointer-events-none disabled:opacity-45',

          // Filled Apple actions — solid colour + white label (tokens only, no hex)
          variant === 'primary' && [
            'bg-brand-gradient text-action-foreground',
            'hover:brightness-110',
          ],
          variant === 'run' && ['bg-action-run text-action-foreground', 'hover:brightness-110'],
          variant === 'edit' && ['bg-action-edit text-action-foreground', 'hover:brightness-110'],
          variant === 'delete' && [
            'bg-action-delete text-action-foreground',
            'hover:brightness-110',
          ],

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

          // Sizes (skipped for `unstyled`)
          !unstyled && size === 'sm' && 'h-7 px-2.5 text-xs',
          !unstyled && size === 'md' && 'h-8 px-3.5 text-sm',
          !unstyled && size === 'lg' && 'h-10 px-5 text-sm',

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
