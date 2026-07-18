"use client";

// The desk: a large dark surface plane behind the notebook plus procedural
// dressing (pencil, eraser, two tape bits, a coffee ring) placed OUTSIDE the
// book's focus area. All the dressing is baked into ONE vertex-coloured merged
// geometry -> a single draw call; each piece carries its own colour in a per-
// vertex `color` attribute so the one MeshStandardMaterial can show them all.
// The desk surface uses a quiet custom felt material (deskMaterial); the dressing
// keeps its low-contrast vertex-coloured MeshStandardMaterial (the coffee ring
// becomes a real stain decal later).
//
// The merged geometry + its material are built imperatively (they need geometry
// merging + baked transforms), so they are disposed explicitly on unmount; the
// desk-surface material is disposed with the plane it drives.

import { useEffect, useMemo } from "react";
import {
  BoxGeometry,
  type BufferGeometry,
  Color,
  ConeGeometry,
  CylinderGeometry,
  Float32BufferAttribute,
  MeshStandardMaterial,
  PlaneGeometry,
  RingGeometry,
} from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

import { createDeskMaterial } from "@/desk/deskMaterial";

const SURFACE_Z = -0.3;
const DRESSING_Z = -0.16;

// sRGB placeholder tones -> linear via three colour management (Color decodes).
const PENCIL_BODY = "#d8a13a";
const PENCIL_WOOD = "#b5895a";
const ERASER_PINK = "#d98fa4";
const TAPE_GREY = "#c8ccce";
const COFFEE_BROWN = "#6b4a2c";

// Write one flat colour into a geometry's per-vertex `color` attribute so it
// survives the merge (mergeGeometries needs matching attributes on every input;
// the primitives all already carry position/normal/uv).
function tint(geo: BufferGeometry, hex: string): BufferGeometry {
  const c = new Color(hex);
  const n = geo.getAttribute("position").count;
  const arr = new Float32Array(n * 3);
  for (let i = 0; i < n; i += 1) {
    arr[i * 3] = c.r;
    arr[i * 3 + 1] = c.g;
    arr[i * 3 + 2] = c.b;
  }
  geo.setAttribute("color", new Float32BufferAttribute(arr, 3));
  return geo;
}

// Build every dressing piece, bake its placement into the vertices, tint it, and
// merge to one geometry. Positions sit off to the sides / bottom, clear of the
// notebook's centred focus area.
function buildDressing(): BufferGeometry {
  const parts: BufferGeometry[] = [];

  // Pencil: a thin cylinder body laid along x with a cone tip. rotateZ(-90) maps
  // the default +y axis onto +x.
  const body = new CylinderGeometry(0.03, 0.03, 1.3, 12);
  body.rotateZ(-Math.PI / 2);
  body.translate(-1.9, -0.5, DRESSING_Z);
  parts.push(tint(body, PENCIL_BODY));

  const tip = new ConeGeometry(0.03, 0.18, 12);
  tip.rotateZ(-Math.PI / 2);
  tip.translate(-1.9 + 0.65 + 0.09, -0.5, DRESSING_Z);
  parts.push(tint(tip, PENCIL_WOOD));

  // Eraser: a small box, upper-left.
  const eraser = new BoxGeometry(0.2, 0.11, 0.11);
  eraser.rotateZ(0.25);
  eraser.translate(-1.7, 0.7, DRESSING_Z);
  parts.push(tint(eraser, ERASER_PINK));

  // Tape bits: two thin quads, right side, slightly rotated.
  const tapeA = new PlaneGeometry(0.28, 0.18);
  tapeA.rotateZ(-0.4);
  tapeA.translate(1.85, 0.5, DRESSING_Z);
  parts.push(tint(tapeA, TAPE_GREY));

  const tapeB = new PlaneGeometry(0.24, 0.16);
  tapeB.rotateZ(0.5);
  tapeB.translate(1.6, 0.1, DRESSING_Z);
  parts.push(tint(tapeB, TAPE_GREY));

  // Coffee ring: a flat ring quad, lower-right (a stain decal later).
  const ring = new RingGeometry(0.18, 0.26, 32);
  ring.translate(1.7, -0.85, DRESSING_Z);
  parts.push(tint(ring, COFFEE_BROWN));

  const merged = mergeGeometries(parts, false);
  for (const p of parts) p.dispose();
  return merged;
}

function Dressing() {
  const geo = useMemo(() => buildDressing(), []);
  const mat = useMemo(
    () => new MeshStandardMaterial({ vertexColors: true, roughness: 0.9, metalness: 0 }),
    [],
  );
  useEffect(
    () => () => {
      geo.dispose();
      mat.dispose();
    },
    [geo, mat],
  );
  return <mesh geometry={geo} material={mat} />;
}

export default function Desk() {
  const surfaceMat = useMemo(() => createDeskMaterial(), []);
  useEffect(() => () => surfaceMat.dispose(), [surfaceMat]);
  return (
    <group>
      {/* Quiet felt desk surface behind the notebook. */}
      <mesh position={[0, 0, SURFACE_Z]} material={surfaceMat}>
        <planeGeometry args={[24, 24]} />
      </mesh>
      <Dressing />
    </group>
  );
}
