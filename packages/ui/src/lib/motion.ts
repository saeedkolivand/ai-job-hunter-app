/**
 * Motion Design System
 * ─────────────────────────────────────────────────────────────────────────
 * Single source of truth for all animation values.
 *
 * Usage:
 *   import { transition, variants, stagger } from '@/lib/motion';
 *
 *   <motion.div {...variants.fadeSlideUp} transition={transition.normal} />
 *   <motion.ul variants={stagger.container}>
 *     <motion.li variants={stagger.item} />
 *   </motion.ul>
 *
 * Principles:
 *   - Every animation duration/easing comes from here, never inline.
 *   - Overlay/modal transitions are distinct from content transitions.
 *   - All values respect prefers-reduced-motion via `resolveTransition`.
 * ─────────────────────────────────────────────────────────────────────────
 */

// ── Easing ────────────────────────────────────────────────────────────────

export const ease = {
  /** Primary smooth ease — most UI transitions. */
  smooth: [0.22, 1, 0.36, 1] as const,
  /** Standard ease-out — simple fades and slides. */
  out: 'easeOut' as const,
  /** Spring — layout shifts and interactive elements like nav pills. */
  spring: { type: 'spring', stiffness: 380, damping: 32 } as const,
  /** Gentle spring — less bouncy, for larger panels. */
  springGentle: { type: 'spring', stiffness: 260, damping: 30 } as const,
} as const;

// ── Durations ─────────────────────────────────────────────────────────────

export const duration = {
  instant: 0.08,
  fast: 0.12,
  normal: 0.18,
  relaxed: 0.22,
  slow: 0.35,
  glacial: 0.5,
} as const;

// ── Pre-built transition objects ─────────────────────────────────────────
// Pass directly: `transition={transition.normal}`

export const transition = {
  fast: { duration: duration.fast, ease: ease.smooth },
  normal: { duration: duration.normal, ease: ease.smooth },
  relaxed: { duration: duration.relaxed, ease: ease.smooth },
  slow: { duration: duration.slow, ease: ease.smooth },
  spring: ease.spring,
  overlay: { duration: duration.fast },
  /** Modal panel (delayed slightly to let overlay appear first) */
  modal: { duration: duration.normal, ease: ease.smooth, delay: 0.05 },
  /** Page-level transitions */
  page: { duration: duration.relaxed, ease: ease.smooth },
  /** Layout selection ring — OptionTile, PerformancePreferences */
  selection: { type: 'spring', stiffness: 300, damping: 30 },
  /** Continuous spinner rotation */
  spin: { duration: 1, repeat: Infinity, ease: 'linear' },
  /** Pulsing / breathing ambient animation */
  pulse: { duration: 1.5, repeat: Infinity, ease: 'easeInOut' },
  /** Slow continuous rotation — large loader rings (half the speed of `spin`) */
  spinSlow: { duration: 2, repeat: Infinity, ease: 'linear' },
  /** Gentle floating / breathing motion — hero icons (slower than `pulse`) */
  breathe: { duration: 3, repeat: Infinity, ease: 'easeInOut' },
  /** Expanding ring ripple / ping halo */
  ping: { duration: 2, repeat: Infinity },
  /** Progress bar fill */
  progress: { duration: 0.8, ease: 'easeOut' },
  /** Data / score bar fill with smooth ease */
  dataBar: { duration: 0.7, ease: ease.smooth },
  /** Atmospheric background blob — first layer */
  blob1: { duration: 0.7, ease: 'easeOut' },
  /** Atmospheric background blob — second layer */
  blob2: { duration: 0.8, ease: 'easeOut', delay: 0.08 },
  /** Atmospheric background blob — third layer */
  blob3: { duration: 0.9, ease: 'easeOut', delay: 0.12 },
  /** Fake indeterminate progress for scraping / medium AI tasks */
  fakeProgress: { duration: 12, ease: 'easeOut' },
  /** Fake indeterminate progress for longer AI generation tasks */
  fakeProgressSlow: { duration: 20, ease: 'easeOut' },
} as const;

/** Returns a list-item entrance transition with a capped stagger delay. */
export const staggeredItem = (index: number, maxDelay = 0.2) => ({
  ...transition.normal,
  delay: Math.min(index * 0.01, maxDelay),
});

/**
 * `transition.normal` with an explicit entrance delay — for sequenced reveals
 * (headings → content → actions) where the delay is hand-tuned, not index-based.
 */
export const withDelay = (delay: number) => ({ ...transition.normal, delay });

// ── Animation variant sets ────────────────────────────────────────────────
// Spread onto motion elements: `{...variants.fadeSlideUp}`

export const variants = {
  fadeSlideUp: {
    initial: { opacity: 0, y: 8 },
    animate: { opacity: 1, y: 0 },
    exit: { opacity: 0, y: -6 },
  },
  fadeSlideDown: {
    initial: { opacity: 0, y: -8 },
    animate: { opacity: 1, y: 0 },
    exit: { opacity: 0, y: 4 },
  },
  expand: {
    initial: { height: 0, opacity: 0 },
    animate: { height: 'auto', opacity: 1 },
    exit: { height: 0, opacity: 0 },
  },
  scale: {
    initial: { opacity: 0, scale: 0.97, y: -10 },
    animate: { opacity: 1, scale: 1, y: 0 },
    exit: { opacity: 0, scale: 0.97, y: -10 },
  },
  scaleFade: {
    initial: { opacity: 0, scale: 0.95 },
    animate: { opacity: 1, scale: 1 },
    exit: { opacity: 0, scale: 0.95 },
  },
  overlay: {
    initial: { opacity: 0 },
    animate: { opacity: 1 },
    exit: { opacity: 0 },
  },
  pageIn: {
    initial: { opacity: 0, y: 14 },
    animate: { opacity: 1, y: 0 },
    exit: { opacity: 0 },
  },
  slideRight: {
    initial: { opacity: 0, x: -12 },
    animate: { opacity: 1, x: 0 },
    exit: { opacity: 0, x: 12 },
  },
} as const;

// ── Stagger system ────────────────────────────────────────────────────────
// For animating lists where children enter one-by-one.
//
// Usage:
//   <motion.ul variants={stagger.container} initial="hidden" animate="show">
//     {items.map(i => <motion.li key={i} variants={stagger.item} />)}
//   </motion.ul>

export const stagger = {
  container: {
    hidden: { opacity: 0 },
    show: {
      opacity: 1,
      transition: { staggerChildren: 0.05, delayChildren: 0.05 },
    },
  },
  containerFast: {
    hidden: { opacity: 0 },
    show: {
      opacity: 1,
      transition: { staggerChildren: 0.03, delayChildren: 0.02 },
    },
  },
  item: {
    hidden: { opacity: 0, y: 8 },
    show: { opacity: 1, y: 0, transition: { duration: duration.normal, ease: ease.smooth } },
  },
  itemScale: {
    hidden: { opacity: 0, scale: 0.95 },
    show: { opacity: 1, scale: 1, transition: { duration: duration.normal, ease: ease.smooth } },
  },
} as const;

// ── Hover animation presets ────────────────────────────────────────────────
// Pass to `whileHover` on interactive elements.

export const hover = {
  lift: { y: -2, transition: { duration: duration.fast, ease: ease.smooth } },
  scale: { scale: 1.02, transition: { duration: duration.fast, ease: ease.smooth } },
  glow: { filter: 'brightness(1.1)', transition: { duration: duration.fast } },
} as const;

export const tap = {
  shrink: { scale: 0.97 },
  press: { scale: 0.95 },
} as const;

// ── Reduced motion helper ─────────────────────────────────────────────────
// Use to resolve transitions respecting user preference at runtime.
// Motion library also respects this automatically when using animate/exit.

export function prefersReducedMotion(): boolean {
  if (typeof window === 'undefined') return false;
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/**
 * Returns the given transition, or an instant one if the user prefers
 * reduced motion. Use this when building custom imperative animations.
 */
export function resolveTransition<T extends object>(t: T): T | { duration: 0 } {
  return prefersReducedMotion() ? { duration: 0 } : t;
}
