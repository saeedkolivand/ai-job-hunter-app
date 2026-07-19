// Procedural stand-in crown for the splash VAT (M3). The real hero asset is a
// Houdini FLIP crown baked to a VAT -- a DCC deliverable that does not exist yet
// (DCC ownership is an open ADR-0016 item). This is the placeholder bake: an
// analytic milk-crown (a ring of displaced verts rising then falling with radial
// falloff), baked ONCE at load into a small float DataTexture and played back
// through the SAME in-house VAT decode path the real bake will drop into. Visually
// a placeholder; contractually identical (baked position per frame per vertex).
//
// This file owns only the PURE, three-free displacement math (unit-tested). The
// component (SplashCrown.tsx) reads its rest geometry, calls crownVertexAt per
// (frame, vertex), and packs the result into a DataTexture.

const TWO_PI = Math.PI * 2;

// Bake dimensions (kept small -- the texture is generated at runtime, so it ships
// 0 bytes and never touches the <=10 MB asset budget). A RingGeometry at these
// segment counts is the rest mesh; its vertex count is the texture width.
export const CROWN_RADIUS = 7;
export const CROWN_RINGS = 12;
export const CROWN_SEGMENTS = 40;
export const CROWN_FRAMES = 64;

function clamp01(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

function smoothstep(edge0: number, edge1: number, x: number): number {
  if (edge0 === edge1) return x < edge0 ? 0 : 1;
  const t = clamp01((x - edge0) / (edge1 - edge0));
  return t * t * (3 - 2 * t);
}

function mix(a: number, b: number, x: number): number {
  return a + (b - a) * x;
}

// Baked crown position for a rest vertex at (radius, angle) at a normalized frame
// position, written into `out` (caller-owned). PURE + deterministic so the bake
// never drifts across sessions (consistent with the scrub contract). The crown is
// FLAT at frame 0 and the last frame (env = 0 there), rising to a scalloped rim +
// centre jet at mid -- a splash that erupts and falls back.
export function crownVertexAt(
  radius: number,
  angle: number,
  frame: number,
  frames: number,
  out: [number, number, number],
): void {
  const fp = frames <= 1 ? 0 : clamp01(frame / (frames - 1));
  const env = Math.sin(Math.PI * fp); // 0 at start/end, 1 at mid: rise then fall
  const rise = Math.pow(env, 0.7);

  // Expanding rim ring: grows from a tight splash to the full radius then settles.
  const rim = mix(0.15, 1, smoothstep(0, 0.55, fp)) * CROWN_RADIUS;
  const width = CROWN_RADIUS * 0.28;
  const d = radius - rim;
  const g = Math.exp(-(d * d) / (2 * width * width));
  const spikes = 1 + 0.35 * Math.cos(CROWN_SEGMENTS * 0.5 * angle); // scalloped rim

  // Central jet: a thin column that shoots up as the crown opens then recedes.
  const jw = CROWN_RADIUS * 0.14;
  const centerJet =
    Math.exp(-(radius * radius) / (2 * jw * jw)) *
    smoothstep(0.1, 0.45, fp) *
    (1 - smoothstep(0.5, 0.9, fp));

  const height = rise * (g * spikes * CROWN_RADIUS * 0.5 + centerJet * CROWN_RADIUS * 0.7);
  const expand = 1 + 0.16 * rise; // slight horizontal spread as it erupts
  out[0] = Math.cos(angle) * radius * expand;
  out[1] = height;
  out[2] = Math.sin(angle) * radius * expand;
}

// Normalize an atan2 angle into [0, 2PI) -- helper for the component when it
// derives (radius, angle) from a rest vertex position.
export function normalizeAngle(a: number): number {
  let x = a % TWO_PI;
  if (x < 0) x += TWO_PI;
  return x;
}
