"use client";

// Renders one doodle from data/doodles.json as ink strokes in GL. Two modes,
// chosen by the `drawOn` prop:
//
//   drawOn present  -> HERO draw-on. Each stroke is its own dashed Line2 (fat
//                      line, worldUnits) so it can reveal independently. The
//                      dash is sized dashSize = gapSize = stroke length, and the
//                      dashOffset is a PURE function of the journey t mapped
//                      through { t0, t1 } -> 0..1 progress (scrub-safe both
//                      directions, computed each frame in a priority-0 useFrame
//                      from journeyStore.getState().t). At progress 0 the gap
//                      covers the whole line (hidden); at 1 the dash does (drawn).
//
//   drawOn absent   -> STATIC DECOR. Every stroke of the doodle is merged into
//                      ONE LineSegments2 batch (a single draw call). Per-stroke
//                      colour is preserved via vertex colours; width cannot vary
//                      inside one batch, so the batch uses the doodle's average
//                      stroke width -- the documented trade for one draw call.
//
// Both modes boil (ink/boil.ts): the material is boil-patched; the shared boil
// clock it reads is advanced once per frame by the composer (its single
// writer). fill:true shapes render in either mode as a triangulated Mesh
// (THREE.ShapeGeometry earcut) in the fill colour, seated a hair behind the
// strokes so the outline draws on top.
//
// Per-frame cost: uniform-value writes only (each hero stroke's dashOffset).
// No setPositions, no geometry rebuild, no allocation.

import { useEffect, useMemo, useRef } from "react";
import {
  Color,
  DoubleSide,
  type Group,
  Mesh,
  MeshBasicMaterial,
  type Object3D,
  Shape,
  ShapeGeometry,
} from "three";
import { Line2 } from "three/addons/lines/Line2.js";
import { LineGeometry } from "three/addons/lines/LineGeometry.js";
import { LineMaterial } from "three/addons/lines/LineMaterial.js";
import { LineSegments2 } from "three/addons/lines/LineSegments2.js";
import { LineSegmentsGeometry } from "three/addons/lines/LineSegmentsGeometry.js";
import { useFrame } from "@react-three/fiber";

import doodlesData from "@/data/doodles.json";
import { journeyStore } from "@/engine/store";
import { BOIL_AMP, patchBoil } from "@/ink/boil";
import { PALETTE } from "@/ink/palette";

interface DoodleStroke {
  pts: number[];
  w?: number;
  color?: string;
  dashed?: boolean;
  fill?: boolean;
}
interface Doodle {
  name: string;
  viewBox: [number, number];
  strokes: DoodleStroke[];
}

const DOODLES = doodlesData as unknown as Doodle[];

export interface DrawOn {
  t0: number;
  t1: number;
}
export interface InkStrokesProps {
  name: string;
  position?: [number, number, number];
  rotation?: [number, number, number];
  scale?: number;
  drawOn?: DrawOn;
}

// A doodle stroke colour resolves to its own hex when present, else page ink.
// `var(--ink)` / null / undefined all fall back to ink; THREE.Color parses the
// sRGB hex and decodes to linear working space (correct for both the diffuse
// uniform and the per-vertex colour buffer).
function strokeColor(raw?: string): Color {
  return new Color(raw && raw.startsWith("#") ? raw : PALETTE.ink);
}

interface Built {
  objects: Object3D[];
  dashItems: { mat: LineMaterial; len: number }[];
  disposables: { dispose(): void }[];
}

export default function InkStrokes({
  name,
  position = [0, 0, 0],
  rotation = [0, 0, 0],
  scale = 1,
  drawOn,
}: InkStrokesProps) {
  // Geometry depends only on the doodle, its scale, and the mode (draw vs
  // decor) -- NOT on t0/t1, which only steer the per-frame dash mapping. Reading
  // them through a ref keeps an inline `drawOn={{...}}` prop from rebuilding the
  // buffers every render.
  const mode = drawOn ? "draw" : "decor";
  const drawRef = useRef(drawOn);
  drawRef.current = drawOn;
  const groupRef = useRef<Group | null>(null);

  const built = useMemo<Built>(() => {
    const objects: Object3D[] = [];
    const dashItems: { mat: LineMaterial; len: number }[] = [];
    const disposables: { dispose(): void }[] = [];

    const doodle = DOODLES.find((d) => d.name === name);
    if (!doodle) {
      if (typeof console !== "undefined") console.warn("InkStrokes: unknown doodle " + name);
      return { objects, dashItems, disposables };
    }

    const [vw, vh] = doodle.viewBox;
    const cx = vw / 2;
    const cy = vh / 2;
    // viewBox px -> local coords: centred, y flipped to three's up axis. The
    // `scale` prop (on the group) converts local px to world units.
    const lx = (x: number) => x - cx;
    const ly = (y: number) => cy - y;
    const ampLocal = BOIL_AMP / (scale || 1);

    const lineStrokes = doodle.strokes.filter((s) => !s.fill);
    const fillStrokes = doodle.strokes.filter((s) => s.fill);

    if (mode === "draw") {
      // One dashed Line2 per stroke so each reveals on its own timeline.
      for (const s of lineStrokes) {
        const pts = s.pts;
        if (pts.length < 4) continue;
        const pos: number[] = [];
        let len = 0;
        let px = 0;
        let py = 0;
        for (let i = 0; i + 1 < pts.length; i += 2) {
          const rawX = pts[i];
          const rawY = pts[i + 1];
          if (rawX === undefined || rawY === undefined) continue;
          const x = lx(rawX);
          const y = ly(rawY);
          pos.push(x, y, 0);
          if (i > 0) len += Math.hypot(x - px, y - py);
          px = x;
          py = y;
        }
        const geo = new LineGeometry();
        geo.setPositions(pos);
        const mat = new LineMaterial({
          color: strokeColor(s.color),
          worldUnits: true,
          linewidth: (s.w ?? 2) * (scale || 1),
          dashed: true,
          dashScale: 1,
          dashSize: len,
          gapSize: len,
          dashOffset: len, // start fully hidden; the useFrame drives it
        });
        patchBoil(mat, ampLocal);
        const line = new Line2(geo, mat);
        line.computeLineDistances();
        line.renderOrder = 2;
        objects.push(line);
        dashItems.push({ mat, len });
        disposables.push(geo, mat);
      }
    } else {
      // Merge every stroke into ONE LineSegments2 batch (one draw call).
      const segPos: number[] = [];
      const segCol: number[] = [];
      let wSum = 0;
      let wN = 0;
      for (const s of lineStrokes) {
        const pts = s.pts;
        if (pts.length < 4) continue;
        const c = strokeColor(s.color);
        wSum += s.w ?? 2;
        wN += 1;
        for (let i = 0; i + 3 < pts.length; i += 2) {
          const x0 = pts[i];
          const y0 = pts[i + 1];
          const x1 = pts[i + 2];
          const y1 = pts[i + 3];
          if (x0 === undefined || y0 === undefined || x1 === undefined || y1 === undefined) continue;
          segPos.push(lx(x0), ly(y0), 0, lx(x1), ly(y1), 0);
          segCol.push(c.r, c.g, c.b, c.r, c.g, c.b);
        }
      }
      if (segPos.length) {
        const geo = new LineSegmentsGeometry();
        geo.setPositions(segPos);
        geo.setColors(segCol);
        const mat = new LineMaterial({
          color: 0xffffff, // white base; per-stroke colour comes from vertexColors
          worldUnits: true,
          linewidth: (wN ? wSum / wN : 2) * (scale || 1),
          vertexColors: true,
        });
        patchBoil(mat, ampLocal);
        const seg = new LineSegments2(geo, mat);
        seg.renderOrder = 1;
        objects.push(seg);
        disposables.push(geo, mat);
      }
    }

    // fill:true polygons -> triangulated meshes, seated just behind the strokes.
    for (const s of fillStrokes) {
      const pts = s.pts;
      if (pts.length < 6) continue;
      const p0 = pts[0];
      const p1 = pts[1];
      if (p0 === undefined || p1 === undefined) continue;
      const shp = new Shape();
      shp.moveTo(lx(p0), ly(p1));
      for (let i = 2; i + 1 < pts.length; i += 2) {
        const xi = pts[i];
        const yi = pts[i + 1];
        if (xi === undefined || yi === undefined) continue;
        shp.lineTo(lx(xi), ly(yi));
      }
      const geo = new ShapeGeometry(shp);
      const mat = new MeshBasicMaterial({ color: strokeColor(s.color), side: DoubleSide });
      const mesh = new Mesh(geo, mat);
      mesh.position.z = -0.01;
      objects.push(mesh);
      disposables.push(geo, mat);
    }

    return { objects, dashItems, disposables };
  }, [name, scale, mode]);

  useEffect(() => {
    const { disposables } = built;
    return () => {
      for (const d of disposables) d.dispose();
    };
  }, [built]);

  // Priority 0 (draw-on only): map t -> dash progress. Runs before the
  // composer's priority-1 render, so the frame draws with fresh uniforms. The
  // shared boil clock is NOT advanced here -- the composer is its single
  // writer (see post/composer.tsx). Pure f(t): scrubbing backwards un-draws
  // the strokes.
  useFrame(() => {
    const d = drawRef.current;
    if (!d || built.dashItems.length === 0) return;
    const t = journeyStore.getState().t;
    const span = d.t1 - d.t0;
    const raw = span > 1e-6 ? (t - d.t0) / span : t >= d.t1 ? 1 : 0;
    const prog = raw < 0 ? 0 : raw > 1 ? 1 : raw;
    for (const it of built.dashItems) it.mat.dashOffset = it.len * (1 - prog);
    // Draw-call cull: at prog 0 every stroke's dash gap already covers its whole
    // line, so the doodle is 100% invisible yet each stroke still costs a draw
    // call. Hiding the group there submits zero draws for a not-yet-drawn (or
    // scrubbed-back-before-window) doodle -- the win in the co-mounted beat
    // overlap, where a neighbour beat's doodles sit pre-window. Pure f(t) and
    // scrub-safe: visibility is a function of t only, and it flips exactly at t0
    // where the strokes (and fills) are already invisible, so nothing on screen
    // changes -- only off-screen/hidden geometry stops being drawn.
    if (groupRef.current) groupRef.current.visible = prog > 0;
  });

  return (
    <group ref={groupRef} position={position} rotation={rotation} scale={scale}>
      {built.objects.map((o, i) => (
        <primitive key={i} object={o} dispose={null} />
      ))}
    </group>
  );
}
