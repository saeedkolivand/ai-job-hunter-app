// Shared clamp01, used everywhere a playhead-shaped value is clamped to
// [0, 1]. Guards against non-finite input (NaN, +-Infinity) by resolving to 0
// -- so a stray NaN upstream (a 0/0 division, a bad DOM read) can never leak
// through as "NaN" in formatted chrome text, a NaN scene index, or a NaN
// scroll offset; every comparison against a bare NaN is false, so an
// un-guarded ternary chain silently falls through to returning the NaN
// itself.
export function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return value < 0 ? 0 : value > 1 ? 1 : value;
}
