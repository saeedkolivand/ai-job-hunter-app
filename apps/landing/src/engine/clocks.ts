// Stepped clocks for hand-drawn motion. The ink line-boil must NOT wobble at
// render framerate -- it reads as digital jitter instead of a hand redrawing
// the line. boilTime quantizes elapsed seconds to a low step rate (the tier's
// boilFps, ~8-10) so the vertex-shader noise only advances a few times a
// second. Feed the result into the boil uniform; identical inputs give
// identical outputs, so it stays scrub-safe.

export function boilTime(elapsed: number, fps: number): number {
  if (fps <= 0) return elapsed;
  return Math.floor(elapsed * fps) / fps;
}
