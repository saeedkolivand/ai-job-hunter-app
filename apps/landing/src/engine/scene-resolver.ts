// Pure playhead -> scene resolver. NO three / NO DOM imports (unit-testable in a
// plain node environment). Expands ADR-0016's approximate % ranges into the
// skill's half-open intervals [lo, hi) so adjacent scenes never overlap at a
// shared boundary; only the final scene is closed at both ends.

export interface Scene {
  readonly index: number;
  readonly id: string; // hash anchor / deep-link target
  readonly act: string; // act title (ASCII)
  readonly lo: number;
  readonly hi: number; // half-open [lo, hi); scene 8 is closed [0.95, 1.00]
}

export const SCENES: readonly Scene[] = [
  { index: 0, id: "cold-open", act: "Cold open", lo: 0.0, hi: 0.05 },
  { index: 1, id: "the-canyon", act: "The canyon", lo: 0.05, hi: 0.3 },
  { index: 2, id: "the-surface", act: "The surface", lo: 0.3, hi: 0.38 },
  { index: 3, id: "the-deep", act: "The deep", lo: 0.38, hi: 0.52 },
  { index: 4, id: "blackout", act: "Blackout", lo: 0.52, hi: 0.58 },
  { index: 5, id: "the-catch", act: "The catch", lo: 0.58, hi: 0.64 },
  { index: 6, id: "the-ascent", act: "The ascent", lo: 0.64, hi: 0.85 },
  { index: 7, id: "dawn", act: "Dawn", lo: 0.85, hi: 0.95 },
  { index: 8, id: "finale", act: "Finale/credits", lo: 0.95, hi: 1.0 },
];

function clamp01(t: number): number {
  return t < 0 ? 0 : t > 1 ? 1 : t;
}

// Active scene index for a playhead value. Boundaries resolve to the higher
// scene (half-open [lo, hi)); t === 1 lands in the closed final scene.
export function resolveScene(t: number): number {
  const c = clamp01(t);
  for (let i = 0; i < SCENES.length; i++) {
    const s = SCENES[i];
    if (s && c >= s.lo && c < s.hi) return i;
  }
  return SCENES.length - 1;
}

// Per-scene local progress sp = (t - lo) / (hi - lo), clamped to [0, 1].
export function sceneProgress(t: number, index: number): number {
  const s = SCENES[index];
  if (!s) return 0;
  return clamp01((t - s.lo) / (s.hi - s.lo));
}

// Playhead t at a scene's range start -- used to reseed after a mode transition
// and for hash deep-links.
export function sceneStartT(index: number): number {
  const s = SCENES[index];
  return s ? s.lo : 0;
}

// Resolve a hash id (e.g. "the-deep") to its scene, or undefined.
export function sceneById(id: string): Scene | undefined {
  return SCENES.find((s) => s.id === id);
}
