// In-house VAT (Vertex Animation Texture) decode contract. `three-vat` does not
// exist on npm and the only real lib excludes our three pin (see webgl-standards
// "VAT contract"), so we own the ~decode ourselves -- it is a handful of
// deterministic texture-sampling math we need for the scrub contract anyway.
//
// This module owns the PURE, three-free index/interpolation math (unit-tested).
// The GPU side is a vertex shader (shaders/vat-crown-shader.ts) that samples the
// baked position texture: each ROW is one frame, each COLUMN is one baked vertex.
// The CPU computes the deterministic frame pair + blend from scene progress and
// hands them to the shader as uniforms, so the whole decode stays a pure f(t) --
// deterministic and reversible, the same as the rest of the film.

export interface VatMeta {
  readonly frames: number; // baked frame count (texture rows / height)
  readonly vertices: number; // baked vertex count (texture columns / width)
  readonly duration: number; // scene-progress span the clip plays over, in [0, 1]
}

export interface VatFrameSample {
  a: number; // lower frame row, in [0, frames - 1]
  b: number; // upper frame row, in [0, frames - 1]
  blend: number; // lerp weight a -> b, in [0, 1]
}

// Deterministic two-frame sample + blend for a scene progress in [0, 1], written
// into `out` (caller-owned) -- the splash crown's useFrame loop preallocates ONE
// VatFrameSample and reuses it every frame instead of allocating a fresh object,
// per the zero-per-frame-allocation discipline. Pure f(progress): the same
// progress always yields the same sample and reversing progress retraces the
// frames exactly (the scrub contract). Inter-frame interpolation is ON -- the
// fractional position between the two nearest baked frames -- so slow scrubs
// read smooth instead of stepping. Guards non-finite input (a stray NaN resolves
// to frame 0, never leaks through). vatFrameIndex() below is a thin fresh-object
// wrapper over this for tests / one-off callers that don't need to avoid the
// allocation; this is the single source of truth for the logic.
export function writeVatFrameIndex(progress: number, frames: number, out: VatFrameSample): void {
  if (frames <= 1) {
    out.a = 0;
    out.b = 0;
    out.blend = 0;
    return;
  }
  const p = Number.isFinite(progress) ? (progress < 0 ? 0 : progress > 1 ? 1 : progress) : 0;
  const pos = p * (frames - 1); // 0 .. frames - 1
  const last = frames - 1;
  const a = Math.floor(pos);
  const aClamped = a < 0 ? 0 : a > last ? last : a;
  const b = aClamped >= last ? last : aClamped + 1;
  const blend = pos - aClamped; // [0, 1)
  out.a = aClamped;
  out.b = b;
  out.blend = blend;
}

// Fresh-object convenience wrapper over writeVatFrameIndex -- see its doc.
export function vatFrameIndex(progress: number, frames: number): VatFrameSample {
  const out: VatFrameSample = { a: 0, b: 0, blend: 0 };
  writeVatFrameIndex(progress, frames, out);
  return out;
}

// The texture V coordinate (row centre) for a baked frame -- the vertex shader
// samples at this V for the given frame row. The half-texel offset lands solidly
// inside the row instead of exactly on a row boundary, so floating-point
// rounding at the edge can never nudge the sample into the adjacent frame's row.
// (The VAT texture samples with NearestFilter -- see SplashCrown.tsx -- so there
// is no GPU linear filtering to bleed between rows; inter-frame interpolation is
// the CPU-computed `blend` weight above, lerped in the decode shader.)
export function frameRowV(frame: number, frames: number): number {
  if (frames <= 0) return 0;
  return (frame + 0.5) / frames;
}

// Map a scene's local progress onto the clip's play window. When duration < 1 the
// crown finishes before the scene ends (then holds on its last frame), which is
// how a splash settles while the camera keeps sinking. Pure f(progress).
export function vatProgress(sceneProgress: number, meta: VatMeta): number {
  const sp = Number.isFinite(sceneProgress)
    ? sceneProgress < 0
      ? 0
      : sceneProgress > 1
        ? 1
        : sceneProgress
    : 0;
  const dur = meta.duration > 0 ? meta.duration : 1;
  const p = sp / dur;
  return p > 1 ? 1 : p;
}
