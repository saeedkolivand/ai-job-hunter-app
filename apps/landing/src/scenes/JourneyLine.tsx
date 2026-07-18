"use client";

// THE 3D JOURNEY LINE -- the signature margin element. A red (palette red)
// fat-line PAIR that rides the camera path from hero to finale: two Line2
// polylines sampled off the journey position curve (evalPose is pure), each
// vertex seated at a fixed CAMERA-SPACE offset of the pose at that t -- to the
// side, below the eye-line, a few units ahead -- so the "current" stretch always
// sits in the viewer's lower margin and reads as ink drawn in the page gutter.
//
// Each rail is a single dashed Line2. Its dashOffset is driven every frame from
// the GLOBAL scroll t: the head is the point at arc-length up to t, and
// dashOffset = total - arcAtHead reveals exactly [start .. head]. Pure f(t) --
// the draw-on follows the viewer's progress EXACTLY and reverses cleanly. A
// tangent-oriented arrowhead (journey-tip / -2, small InkStrokes) is glued to
// each rail's current draw head via the same f(t): billboarded to the lens, spun
// so the arrow POINT rides the head pointing along the forward tangent. Across
// the finale tail [CONV_START, 1] the tail vertices leave the margin and sweep
// into the CTA button, so at t -> 1 both heads land pointing at it.
//
// Cost: 2 rail draws + 2 tip doodles. Per frame: two dashOffset writes + two tip
// transforms, zero allocation (module-scope scratch). Curve sampled coarsely.

import { useEffect, useMemo, useRef } from "react";
import { Color, type Group, Quaternion, Vector3 } from "three";
import { Line2 } from "three/addons/lines/Line2.js";
import { LineGeometry } from "three/addons/lines/LineGeometry.js";
import { LineMaterial } from "three/addons/lines/LineMaterial.js";
import { useFrame } from "@react-three/fiber";

import { evalPose } from "@/engine/journey";
import { journeyStore } from "@/engine/store";
import { BOIL_AMP, patchBoil } from "@/ink/boil";
import InkStrokes from "@/ink/InkStrokes";
import { PALETTE } from "@/ink/palette";
import { CTA_ANCHOR } from "@/scenes/Finale";

const SEGMENTS = 200; // coarse samples per rail (each rail is still ONE draw)
const SIDE = 3.6; // lateral offset from the lens axis (world units)
const DROP = 2.4; // below the camera eye-line -> reads as lower-margin ink
const AHEAD = 8; // seated this far in front of the lens
const CONV_START = 0.95; // finale tail: heads leave the margin, sweep to the CTA
const TIP_SCALE = 0.04; // arrowhead size

// journey-tip's apex (raw (2,0) in its 28x16 viewBox) lands at InkStrokes-local
// (-12, +8); this inner offset seats that apex on the tip group's origin so the
// arrow POINT sits exactly on the moving draw head (and the group pivots on it).
const TIP_OX = 12 * TIP_SCALE;
const TIP_OY = -8 * TIP_SCALE;

interface Rail {
  line: Line2;
  mat: LineMaterial;
  pts: Vector3[];
  cum: number[];
  total: number;
}

// Local smoothstep (pure f) for the convergence sweep.
function smooth(x: number): number {
  const u = x < 0 ? 0 : x > 1 ? 1 : x;
  return u * u * (3 - 2 * u);
}

// Build one rail alongside the camera path. `side` places it left (-1) or right
// (+1) of the lens axis. The tail (t > CONV_START) sweeps from the last margin
// point into a target that flanks the CTA button, so the arrow lands on it.
function buildRail(side: 1 | -1): Rail {
  const target = new Vector3(CTA_ANCHOR[0] + side * 0.35, CTA_ANCHOR[1] - 0.15, CTA_ANCHOR[2]);
  const pts: Vector3[] = [];
  let convAnchor = new Vector3();

  for (let i = 0; i <= SEGMENTS; i++) {
    const t = i / SEGMENTS;
    const pose = evalPose(t); // reuses journey scratch; consumed before next call
    const q = pose.quaternion;
    const r = new Vector3(1, 0, 0).applyQuaternion(q);
    const u = new Vector3(0, 1, 0).applyQuaternion(q);
    const f = new Vector3(0, 0, -1).applyQuaternion(q);
    const margin = new Vector3()
      .copy(pose.position)
      .addScaledVector(r, side * SIDE)
      .addScaledVector(u, -DROP)
      .addScaledVector(f, AHEAD);
    if (t <= CONV_START) {
      pts.push(margin);
      convAnchor = margin;
    } else {
      const c = smooth((t - CONV_START) / (1 - CONV_START));
      pts.push(new Vector3().copy(convAnchor).lerp(target, c));
    }
  }

  // Flatten + cumulative arc length (walk with a prev pointer -- no index access).
  const positions: number[] = [];
  const cum: number[] = [];
  let acc = 0;
  let prev: Vector3 | null = null;
  for (const p of pts) {
    positions.push(p.x, p.y, p.z);
    if (prev) acc += p.distanceTo(prev);
    cum.push(acc);
    prev = p;
  }
  const total = acc;

  const geo = new LineGeometry();
  geo.setPositions(positions);
  const mat = new LineMaterial({
    color: new Color(PALETTE.red),
    worldUnits: true,
    linewidth: 0.08,
    dashed: true,
    dashScale: 1,
    dashSize: total,
    gapSize: total,
    dashOffset: total, // fully hidden; the useFrame reveals it by global t
  });
  patchBoil(mat, BOIL_AMP); // rail is unscaled (world units) -> ampLocal = BOIL_AMP
  const line = new Line2(geo, mat);
  line.computeLineDistances();
  line.renderOrder = 2;
  return { line, mat, pts, cum, total };
}

// Per-frame scratch (never reallocated).
const _qc = new Quaternion();
const _r = new Vector3();
const _u = new Vector3();
const _head = new Vector3();
const _tan = new Vector3();

// Reveal a rail up to the global-t head and glue its tip there. _qc/_r/_u are the
// current lens frame, set once per frame before this is called for each rail.
function updateRail(rail: Rail, tip: Group | null, t: number): void {
  const seg = rail.pts.length - 1;
  const fi = t * seg;
  let i = Math.floor(fi);
  if (i < 0) i = 0;
  if (i > seg - 1) i = seg - 1;
  const fr = fi - i;
  const a = rail.pts[i];
  const b = rail.pts[i + 1];
  const ca = rail.cum[i];
  const cb = rail.cum[i + 1];
  if (!a || !b || ca === undefined || cb === undefined) return;

  _head.lerpVectors(a, b, fr);
  const arc = ca + (cb - ca) * fr;
  rail.mat.dashOffset = rail.total - arc; // reveals [start .. head]

  if (!tip) return;
  tip.visible = t > 0.003; // no pen tip before the line starts drawing
  if (!tip.visible) return;
  tip.position.copy(_head);
  _tan.subVectors(b, a); // forward tangent (direction of increasing t)
  const theta = Math.atan2(_tan.dot(_u), _tan.dot(_r)); // its screen-plane angle
  tip.quaternion.copy(_qc); // billboard to the lens...
  tip.rotateZ(theta); // ...then point the arrow along the tangent
}

export default function JourneyLine() {
  const rails = useMemo(() => [buildRail(-1), buildRail(1)] as const, []);
  const tipL = useRef<Group>(null);
  const tipR = useRef<Group>(null);

  // Manual disposal: the primitives are dispose={null}, so release the two rails'
  // GPU resources when the Canvas tears down (useMemo rebuilds fresh on remount).
  useEffect(
    () => () => {
      for (const rl of rails) {
        rl.line.geometry.dispose();
        rl.mat.dispose();
      }
    },
    [rails],
  );

  // Priority 0 (before the composer's priority-1 render): pure f(t). Derive the
  // lens frame once from evalPose (same pose the composer copies to the camera),
  // then reveal each rail + place its tip.
  useFrame(() => {
    const t = Math.min(1, Math.max(0, journeyStore.getState().t));
    _qc.copy(evalPose(t).quaternion); // journey scratch; copied immediately
    _r.set(1, 0, 0).applyQuaternion(_qc);
    _u.set(0, 1, 0).applyQuaternion(_qc);
    updateRail(rails[0], tipL.current, t);
    updateRail(rails[1], tipR.current, t);
  });

  return (
    <>
      <primitive object={rails[0].line} dispose={null} />
      <primitive object={rails[1].line} dispose={null} />
      <group ref={tipL} visible={false}>
        <InkStrokes name="journey-tip" position={[TIP_OX, TIP_OY, 0]} scale={TIP_SCALE} drawOn={{ t0: 0, t1: 0 }} />
      </group>
      <group ref={tipR} visible={false}>
        <InkStrokes name="journey-tip-2" position={[TIP_OX, TIP_OY, 0]} scale={TIP_SCALE} drawOn={{ t0: 0, t1: 0 }} />
      </group>
    </>
  );
}
