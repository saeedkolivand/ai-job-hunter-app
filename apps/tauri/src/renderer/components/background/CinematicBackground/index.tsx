/**
 * Ambient backdrop — Apple "restraint" pass (A1).
 *
 * The previous cinematic system (aurora ribbons, nebulae, soft streaks, a
 * 900px lerp-smoothed cursor blob, parallax glow orbs, grid texture, film grain
 * and vignette) was removed: the Apple design language forbids decorative
 * gradients and glows, and elevation now comes from surface-color steps +
 * hairlines, not ambient light. The canvas is simply the calm, flat themed
 * `--color-background`, rendered as one static, non-interactive layer (no RAF
 * loop, no pointer listeners, no re-renders — also a performance win).
 */
export function CinematicBackground() {
  return <div aria-hidden className="pointer-events-none fixed inset-0 -z-10 bg-background" />;
}
