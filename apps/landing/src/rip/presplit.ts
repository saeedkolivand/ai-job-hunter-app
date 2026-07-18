// Row-snap corner-tear pre-split. Runs ONCE at load (never per frame): it takes
// a dense grid plane and splits it into two watertight BufferGeometries -- the
// HELD page and the FREE corner piece -- that share a torn seam. A seeded fbm
// polyline seam arcs across the top-right corner; every grid cell is classified
// held or free by its centre; the vertices straddling that boundary are snapped
// exactly onto the seam and duplicated (held keeps the originals, free gets its
// own copies at the same positions), so at rest the two pieces meet with no gap
// and no T-junction. The rip vertex shader (shader-engineer's pass) then reads
// the per-vertex attributes below to gate the tear front and bend the peel; JS
// throws the free piece as a rigid body. Everything here is a pure function of
// (seed, style) -- deterministic, so tests and shaders can rely on it.
//
// Per-vertex attributes written on both pieces:
//   aSeamDist  signed distance to the seam curve. + on the held side, - on the
//              free side, ~0 on the snapped seam vertices. The peel bend weights
//              by |aSeamDist| (bend concentrates near the seam).
//   aSeamArc   0..1 arc position of the nearest seam point -- the tear front
//              sweeps along this as the rip progresses.
//   aEdgeFlag  1 on the snapped seam (torn-edge) vertices, else 0.
//   aNoise     stable per-vertex hash seed for ink/edge jitter.

import {
  BufferGeometry,
  Float32BufferAttribute,
  Uint32BufferAttribute,
} from "three";

import { PAGE_H, PAGE_W } from "@/engine/pages";

// Grid resolution: 96 x 128 segments -> 97 x 129 vertices.
const COLS = 96;
const ROWS = 128;

// Bounds-safe read. Every index in this builder is structurally in range (grid
// indices), so the fallback never fires; it just satisfies
// noUncheckedIndexedAccess without scattering non-null assertions.
function at(a: ArrayLike<number>, i: number): number {
  const v = a[i];
  return v === undefined ? 0 : v;
}

export interface CornerTearStyle {
  // Where the seam meets the top edge, as a fraction of width in from the
  // top-right corner (0 = at the corner, larger = a wider corner piece).
  topFrac: number;
  // Where the seam meets the right edge, as a fraction of height down from the
  // top-right corner.
  rightFrac: number;
  // Ragged-edge amplitude (fraction of the shorter page dimension).
  jag: number;
  // fbm frequency along the seam (higher = finer raggedness).
  jagFreq: number;
  // How far the seam bulges toward the corner (0 = straight chord, 1 = through
  // the corner). Gives the tear a natural arc.
  bulge: number;
}

export const DEFAULT_CORNER_TEAR: CornerTearStyle = {
  topFrac: 0.3,
  rightFrac: 0.35,
  jag: 0.02,
  jagFreq: 9,
  bulge: 0.45,
};

export interface SeamDescriptor {
  // Flat [x0,y0, x1,y1, ...] seam polyline, top-edge start to right-edge end.
  points: Float32Array;
  // Cumulative arc length at each sample (points.length / 2 entries).
  arc: Float32Array;
  length: number;
  start: [number, number];
  end: [number, number];
}

export interface PreSplit {
  held: BufferGeometry;
  free: BufferGeometry;
  seam: SeamDescriptor;
}

// Deterministic hash in [0,1) from an integer.
function hash1(n: number): number {
  const s = Math.sin(n * 127.1 + 311.7) * 43758.5453123;
  return s - Math.floor(s);
}

// Value-noise fbm along a 1D parameter, seeded.
function fbm(x: number, seed: number): number {
  let sum = 0;
  let amp = 0.5;
  let freq = 1;
  for (let o = 0; o < 4; o++) {
    const xf = x * freq + seed * 17.13;
    const i = Math.floor(xf);
    const f = xf - i;
    const u = f * f * (3 - 2 * f);
    const a = hash1(i + seed * 57);
    const b = hash1(i + 1 + seed * 57);
    sum += (a + (b - a) * u - 0.5) * 2 * amp;
    amp *= 0.5;
    freq *= 2;
  }
  return sum;
}

const SEAM_SAMPLES = 160;

// Build the seam polyline: a quadratic-Bezier arc from the top edge to the right
// edge, pulled toward the corner, plus fbm jag along its normal.
function buildSeam(style: CornerTearStyle, seed: number): SeamDescriptor {
  const halfW = PAGE_W / 2;
  const halfH = PAGE_H / 2;
  const start: [number, number] = [halfW - style.topFrac * PAGE_W, halfH];
  const end: [number, number] = [halfW, halfH - style.rightFrac * PAGE_H];
  const corner: [number, number] = [halfW, halfH];
  // Control point: chord midpoint pulled toward the corner by `bulge`.
  const midX = (start[0] + end[0]) / 2;
  const midY = (start[1] + end[1]) / 2;
  const cx = midX + (corner[0] - midX) * style.bulge;
  const cy = midY + (corner[1] - midY) * style.bulge;

  const jagAmp = style.jag * Math.min(PAGE_W, PAGE_H);
  const n = SEAM_SAMPLES;
  const points = new Float32Array(n * 2);
  const arc = new Float32Array(n);

  // First pass: base Bezier points.
  for (let k = 0; k < n; k++) {
    const u = k / (n - 1);
    const iu = 1 - u;
    // Quadratic Bezier B(u) = iu^2*start + 2*iu*u*C + u^2*end.
    let px = iu * iu * start[0] + 2 * iu * u * cx + u * u * end[0];
    let py = iu * iu * start[1] + 2 * iu * u * cy + u * u * end[1];
    // Tangent (derivative) for the normal direction.
    const tx = 2 * iu * (cx - start[0]) + 2 * u * (end[0] - cx);
    const ty = 2 * iu * (cy - start[1]) + 2 * u * (end[1] - cy);
    const tl = Math.hypot(tx, ty) || 1;
    const nx = -ty / tl;
    const ny = tx / tl;
    // fbm jag, faded to zero at both ends so the seam stays pinned to the edges.
    const fade = Math.sin(Math.PI * u);
    const j = fbm(u * style.jagFreq, seed) * jagAmp * fade;
    px += nx * j;
    py += ny * j;
    points[k * 2] = px;
    points[k * 2 + 1] = py;
  }

  // Second pass: cumulative arc length.
  let acc = 0;
  arc[0] = 0;
  for (let k = 1; k < n; k++) {
    const dx = at(points, k * 2) - at(points, (k - 1) * 2);
    const dy = at(points, k * 2 + 1) - at(points, (k - 1) * 2 + 1);
    acc += Math.hypot(dx, dy);
    arc[k] = acc;
  }

  return { points, arc, length: acc, start, end };
}

// Nearest point on the seam polyline to (px,py): distance and the normalized
// arc position of that nearest point.
interface Nearest {
  dist: number;
  arcN: number;
}

function nearestOnSeam(seam: SeamDescriptor, px: number, py: number): Nearest {
  const pts = seam.points;
  const n = pts.length / 2;
  let bestD = Infinity;
  let bestArc = 0;
  for (let k = 0; k < n - 1; k++) {
    const ax = at(pts, k * 2);
    const ay = at(pts, k * 2 + 1);
    const ex = at(pts, (k + 1) * 2) - ax;
    const ey = at(pts, (k + 1) * 2 + 1) - ay;
    const len2 = ex * ex + ey * ey || 1;
    let s = ((px - ax) * ex + (py - ay) * ey) / len2;
    if (s < 0) s = 0;
    else if (s > 1) s = 1;
    const cxp = ax + ex * s;
    const cyp = ay + ey * s;
    const dx = px - cxp;
    const dy = py - cyp;
    const d = dx * dx + dy * dy;
    if (d < bestD) {
      bestD = d;
      const segArc = at(seam.arc, k) + s * (at(seam.arc, k + 1) - at(seam.arc, k));
      bestArc = segArc / (seam.length || 1);
    }
  }
  return { dist: Math.sqrt(bestD), arcN: bestArc };
}

// Nearest actual point on the seam polyline (position, for snapping).
function nearestPoint(
  seam: SeamDescriptor,
  px: number,
  py: number,
): { x: number; y: number } {
  const pts = seam.points;
  const n = pts.length / 2;
  let bestD = Infinity;
  let rx = px;
  let ry = py;
  for (let k = 0; k < n - 1; k++) {
    const ax = at(pts, k * 2);
    const ay = at(pts, k * 2 + 1);
    const ex = at(pts, (k + 1) * 2) - ax;
    const ey = at(pts, (k + 1) * 2 + 1) - ay;
    const len2 = ex * ex + ey * ey || 1;
    let s = ((px - ax) * ex + (py - ay) * ey) / len2;
    if (s < 0) s = 0;
    else if (s > 1) s = 1;
    const cxp = ax + ex * s;
    const cyp = ay + ey * s;
    const dx = px - cxp;
    const dy = py - cyp;
    const d = dx * dx + dy * dy;
    if (d < bestD) {
      bestD = d;
      rx = cxp;
      ry = cyp;
    }
  }
  return { x: rx, y: ry };
}

// Ray-cast point-in-polygon over the free-corner polygon (seam samples + the
// corner). True = inside the free corner piece.
function pointInFree(seam: SeamDescriptor, px: number, py: number): boolean {
  const pts = seam.points;
  const n = pts.length / 2;
  const cornerX = PAGE_W / 2;
  const cornerY = PAGE_H / 2;
  const vx = (idx: number) => (idx < n ? at(pts, idx * 2) : cornerX);
  const vy = (idx: number) => (idx < n ? at(pts, idx * 2 + 1) : cornerY);
  const total = n + 1;
  let inside = false;
  for (let i = 0, j = total - 1; i < total; j = i++) {
    const xi = vx(i);
    const yi = vy(i);
    const xj = vx(j);
    const yj = vy(j);
    const intersects =
      yi > py !== yj > py &&
      px < ((xj - xi) * (py - yi)) / (yj - yi || 1e-9) + xi;
    if (intersects) inside = !inside;
  }
  return inside;
}

export function presplitCornerTear(
  seed = 1,
  style: CornerTearStyle = DEFAULT_CORNER_TEAR,
): PreSplit {
  const nx = COLS + 1;
  const ny = ROWS + 1;
  const vCount = nx * ny;
  const seam = buildSeam(style, seed);

  // Base grid: centred plane in the XY plane, +z front, y up. UV matches the
  // grid; top-right corner (i=COLS, j=ROWS) is where the tear originates.
  const bx = new Float32Array(vCount);
  const by = new Float32Array(vCount);
  const bu = new Float32Array(vCount);
  const bv = new Float32Array(vCount);
  for (let j = 0; j <= ROWS; j++) {
    const v = j / ROWS;
    const y = v * PAGE_H - PAGE_H / 2;
    for (let i = 0; i <= COLS; i++) {
      const u = i / COLS;
      const idx = j * nx + i;
      bx[idx] = u * PAGE_W - PAGE_W / 2;
      by[idx] = y;
      bu[idx] = u;
      bv[idx] = v;
    }
  }

  // Classify each cell by its centre; mark which side touches each vertex.
  const touchHeld = new Uint8Array(vCount);
  const touchFree = new Uint8Array(vCount);
  const cellFree = new Uint8Array(COLS * ROWS);
  for (let j = 0; j < ROWS; j++) {
    for (let i = 0; i < COLS; i++) {
      const c00 = j * nx + i;
      const c01 = (j + 1) * nx + i;
      const cx = (at(bx, c00) + at(bx, c00 + 1)) / 2;
      const cy = (at(by, c00) + at(by, c01)) / 2;
      const free = pointInFree(seam, cx, cy) ? 1 : 0;
      cellFree[j * COLS + i] = free;
      const touch = free ? touchFree : touchHeld;
      touch[c00] = 1;
      touch[c00 + 1] = 1;
      touch[c01] = 1;
      touch[c01 + 1] = 1;
    }
  }

  // Snap boundary vertices (touched by both sides) onto the seam so the torn
  // edge follows the curve exactly and both pieces share the same positions.
  const isEdge = new Uint8Array(vCount);
  for (let idx = 0; idx < vCount; idx++) {
    if (at(touchHeld, idx) && at(touchFree, idx)) {
      isEdge[idx] = 1;
      const near = nearestPoint(seam, at(bx, idx), at(by, idx));
      bx[idx] = near.x;
      by[idx] = near.y;
    }
  }

  // Per-vertex attributes from the final positions.
  const aDist = new Float32Array(vCount);
  const aArc = new Float32Array(vCount);
  const aNoise = new Float32Array(vCount);
  for (let idx = 0; idx < vCount; idx++) {
    const near = nearestOnSeam(seam, at(bx, idx), at(by, idx));
    // Sign from which piece owns the vertex (edge vertices -> 0 distance).
    const sign = at(isEdge, idx) ? 0 : at(touchFree, idx) ? -1 : 1;
    aDist[idx] = sign * near.dist;
    aArc[idx] = near.arcN;
    aNoise[idx] = hash1(idx * 2.399 + 0.5);
  }

  const buildPiece = (wantFree: number): BufferGeometry => {
    const remap = new Int32Array(vCount).fill(-1);
    const pos: number[] = [];
    const nor: number[] = [];
    const uv: number[] = [];
    const sd: number[] = [];
    const sa: number[] = [];
    const ef: number[] = [];
    const nz: number[] = [];
    const index: number[] = [];
    let count = 0;
    const emit = (idx: number): number => {
      let r = at(remap, idx);
      if (r < 0) {
        r = count++;
        remap[idx] = r;
        pos.push(at(bx, idx), at(by, idx), 0);
        nor.push(0, 0, 1);
        uv.push(at(bu, idx), at(bv, idx));
        sd.push(at(aDist, idx));
        sa.push(at(aArc, idx));
        ef.push(at(isEdge, idx));
        nz.push(at(aNoise, idx));
      }
      return r;
    };
    for (let j = 0; j < ROWS; j++) {
      for (let i = 0; i < COLS; i++) {
        if (at(cellFree, j * COLS + i) !== wantFree) continue;
        const c00 = j * nx + i;
        const c01 = (j + 1) * nx + i;
        const r00 = emit(c00);
        const r10 = emit(c00 + 1);
        const r01 = emit(c01);
        const r11 = emit(c01 + 1);
        // CCW winding, front toward +z.
        index.push(r00, r10, r11, r00, r11, r01);
      }
    }
    const geo = new BufferGeometry();
    geo.setAttribute("position", new Float32BufferAttribute(pos, 3));
    geo.setAttribute("normal", new Float32BufferAttribute(nor, 3));
    geo.setAttribute("uv", new Float32BufferAttribute(uv, 2));
    geo.setAttribute("aSeamDist", new Float32BufferAttribute(sd, 1));
    geo.setAttribute("aSeamArc", new Float32BufferAttribute(sa, 1));
    geo.setAttribute("aEdgeFlag", new Float32BufferAttribute(ef, 1));
    geo.setAttribute("aNoise", new Float32BufferAttribute(nz, 1));
    geo.setIndex(new Uint32BufferAttribute(index, 1));
    geo.computeBoundingSphere();
    return geo;
  };

  return {
    held: buildPiece(0),
    free: buildPiece(1),
    seam,
  };
}
