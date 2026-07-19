// Procedural "typed lines" letter impression for the paper storm. Baked ONCE on
// a 2D canvas at first use (NEVER during SSR) and cached as a module singleton --
// a single small texture shared by every sheet instance for the whole session
// (intentionally not disposed per-mesh; it is a shared cached asset, not owned by
// any one PaperStorm mount). No downloaded assets (the <=10 MB budget forbids
// letter images) -- just grey line blocks that read as paragraphs of a rejection
// letter at distance. NOT real text.
//
// Layout is a 2x2 atlas of four distinct letter variants; the storm vertex shader
// picks one cell per instance off aSeed so neighbouring sheets do not read
// identically. Deterministic bake (seeded PRNG) so the impression is identical
// every session -- consistent with the pure-f(t) scrub contract.

import { CanvasTexture, SRGBColorSpace, type Texture } from "three";

const TEX_SIZE = 256; // <= 256^2 per the budget note; 2x2 atlas -> 128px cells
const CELLS = 2;

let cached: Texture | null = null;

// Tiny deterministic PRNG (integer-mix), so the baked letters never drift.
function makeRng(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s ^ (s >>> 15), 0x2c1b3c6d) + 0x9e3779b9) >>> 0;
    return (s >>> 8) / 16777216;
  };
}

// Draw one rejection-letter impression into the [x0, y0] .. [x0+size, y0+size]
// cell: a masthead bar, a short recipient line, a few justified body paragraphs,
// and a signature scrawl. Near-white paper ground with mid-grey ink so the map
// acts as a text-darkening mask over the material's paper tint.
function drawLetter(
  ctx: CanvasRenderingContext2D,
  x0: number,
  y0: number,
  size: number,
  seed: number,
): void {
  const rand = makeRng(seed);
  const pad = size * 0.14;
  const left = x0 + pad;
  const usable = size - pad * 2;
  const bottom = y0 + size - pad;
  let y = y0 + pad;

  // Masthead / letterhead bar.
  ctx.fillStyle = "#8f887b";
  ctx.fillRect(left, y, usable * (0.3 + rand() * 0.14), size * 0.05);
  y += size * 0.12;

  // Date / recipient line (short).
  ctx.fillStyle = "#b7af9f";
  ctx.fillRect(left, y, usable * (0.38 + rand() * 0.2), size * 0.022);
  y += size * 0.085;

  // Body paragraphs: rows of justified-looking lines, last line short.
  const paras = 3 + Math.floor(rand() * 2);
  for (let p = 0; p < paras && y < bottom - size * 0.1; p++) {
    const lines = 2 + Math.floor(rand() * 3);
    for (let l = 0; l < lines && y < bottom - size * 0.06; l++) {
      const last = l === lines - 1;
      const w = last ? usable * (0.3 + rand() * 0.32) : usable * (0.85 + rand() * 0.12);
      ctx.fillStyle = "#a99f8d";
      ctx.fillRect(left, y, Math.min(w, usable), size * 0.02);
      y += size * 0.04;
    }
    y += size * 0.028; // paragraph gap
  }

  // Signature scrawl pinned near the bottom.
  ctx.fillStyle = "#847a6a";
  ctx.fillRect(left, bottom - size * 0.03, usable * (0.22 + rand() * 0.12), size * 0.03);
}

// Return the cached letter texture, baking it on first client call. Returns null
// during SSR (no document) so the storm shader's letter path stays fail-soft.
export function getLetterTexture(): Texture | null {
  if (cached) return cached;
  if (typeof document === "undefined") return null; // never bake during SSR

  const canvas = document.createElement("canvas");
  canvas.width = TEX_SIZE;
  canvas.height = TEX_SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  const cell = TEX_SIZE / CELLS;
  for (let cy = 0; cy < CELLS; cy++) {
    for (let cx = 0; cx < CELLS; cx++) {
      const x0 = cx * cell;
      const y0 = cy * cell;
      ctx.fillStyle = "#f4efe6"; // near-white paper ground for the cell
      ctx.fillRect(x0, y0, cell, cell);
      drawLetter(ctx, x0, y0, cell, 0x9e37 + cx * 131 + cy * 977);
    }
  }

  const tex = new CanvasTexture(canvas);
  tex.colorSpace = SRGBColorSpace; // three's map_fragment decodes to linear
  tex.needsUpdate = true;
  cached = tex;
  return tex;
}
