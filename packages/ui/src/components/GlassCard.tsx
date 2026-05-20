import type { HTMLAttributes } from 'react';
import { cn } from '../lib/cn';

type Tone = 'neutral' | 'violet' | 'indigo' | 'graphite';

export interface GlassCardProps extends HTMLAttributes<HTMLDivElement> {
  tone?: Tone;
  /** Adds the top-edge hairline highlight. Default true. */
  highlight?: boolean;
  /** Adds a soft outer glow. */
  glow?: boolean;
}

const toneClass: Record<Tone, string> = {
  neutral: 'glass-card',
  violet: 'glass-violet',
  indigo: 'glass-indigo',
  graphite: 'glass-graphite',
};

export function GlassCard({
  className,
  tone = 'neutral',
  highlight = true,
  glow = false,
  ...rest
}: GlassCardProps) {
  return (
    <div
      className={cn(
        toneClass[tone],
        'p-5',
        highlight && 'glass-highlight',
        glow && 'glow-subtle',
        className
      )}
      {...rest}
    />
  );
}
