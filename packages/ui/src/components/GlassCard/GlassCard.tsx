import type { HTMLAttributes } from 'react';

import { cn } from '../../lib/cn';

/**
 * Surface tones.
 * - `surface` (default) — the flat Apple content surface: surface-color fill +
 *   1px hairline, NO shadow/blur (elevation = surface-color change). This is the
 *   A1 "restraint" default; every plain `<GlassCard>` is now a flat card.
 * - `glass` / `violet` / `indigo` / `graphite` — opt-in frosted glass, reserved
 *   for hero surfaces (modals, dashboard hero, chrome) that keep the vibrancy.
 * - `neutral` — legacy alias of `glass` (kept so existing explicit usages that
 *   relied on the old glass default don't change).
 */
type Tone = 'surface' | 'glass' | 'neutral' | 'violet' | 'indigo' | 'graphite';

export interface GlassCardProps extends HTMLAttributes<HTMLDivElement> {
  tone?: Tone;
  /** Top-edge hairline highlight — only meaningful on glass tones. Default true. */
  highlight?: boolean;
  /** Adds a soft outer glow (glass tones only). */
  glow?: boolean;
}

const toneClass: Record<Tone, string> = {
  surface: 'surface-card',
  glass: 'glass-card',
  neutral: 'glass-card',
  violet: 'glass-violet',
  indigo: 'glass-indigo',
  graphite: 'glass-graphite',
};

const GLASS_TONES = new Set<Tone>(['glass', 'neutral', 'violet', 'indigo', 'graphite']);

export function GlassCard({
  className,
  tone = 'surface',
  highlight = true,
  glow = false,
  ...rest
}: GlassCardProps) {
  const isGlass = GLASS_TONES.has(tone);
  return (
    <div
      className={cn(
        toneClass[tone],
        'p-5',
        isGlass && highlight && 'glass-highlight',
        isGlass && glow && 'ring-1 ring-brand/20',
        className
      )}
      {...rest}
    />
  );
}
