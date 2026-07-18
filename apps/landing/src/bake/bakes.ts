// The bake-texture singletons, shared by reference the same way engine/uniforms
// exposes uBoil/uRipP. Each entry is a plain { value } holder that starts null
// and is filled in-place by bakeAll (bake.ts) once, at load, behind the loader.
// A material that wants a bake passes the SAME holder object as its sampler
// uniform (e.g. uPaper: bakes.paperAlbedoRough) so it picks up the baked texture
// by reference the instant bakeAll assigns .value -- zero re-wiring, no per-page
// re-bake. The textures live for the whole session (never disposed on unmount);
// only the bake-time quad/materials are torn down (see bake.ts).
//
// Channel layout each bake target packs (authored by shader-engineer; the harness
// only owns the render targets + this contract):
//   paperAlbedoRough  rgb = kraft albedo (linear),      a = roughness
//   paperNormalHeight rg  = tangent-space normal xy,     b = height, a = free
//   stain    1024^2  seeded coffee/ink stain atlas (rgb tint, a = coverage)
//   smudge    512^2  seeded graphite/eraser smudge atlas (r = density)
//   hatch    2048^2  channel-packed pencil hatch: R single / G cross / B heavy
//                    / A blue-noise grain
//
// colorSpace: every target is authored + sampled as raw data (NoColorSpace) --
// albedo is stored linear, the rest are non-color data (roughness, normal,
// height, masks). The material does any decode/lighting; nothing here is a
// display-referred sRGB texture.

import type { Texture } from "three";

export type BakeName =
  | "paperAlbedoRough"
  | "paperNormalHeight"
  | "stain"
  | "smudge"
  | "hatch";

export interface BakeHolder {
  value: Texture | null;
}

export const bakes: Record<BakeName, BakeHolder> = {
  paperAlbedoRough: { value: null },
  paperNormalHeight: { value: null },
  stain: { value: null },
  smudge: { value: null },
  hatch: { value: null },
};
