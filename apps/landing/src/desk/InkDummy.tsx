"use client";

// The ink test prop: a small sphere-on-capsule knick-knack on the desk edge (dev
// proving ground for the ink system; harmless to keep as a desk trinket in prod).
// It exercises the TWO ink material slots end-to-end:
//   - the ink fill (createInkMaterial) on the model itself, and
//   - the inverted-hull outline (createInkOutlineMaterial): the SAME geometry,
//     scaled up slightly, rendered with front faces culled (BackSide) so only a
//     rim shows behind the fill.
// The ink fill (createInkMaterial) is a 3-band toon + tri-planar hatch; the
// outline (createInkOutlineMaterial) shader-extrudes the hull to a screen-
// constant width in CLIP space, so the outline mesh renders at scale 1 (no CPU
// hull inflation). The merged geometry + the two materials are disposed on
// unmount.

import { useEffect, useMemo } from "react";
import { CapsuleGeometry, SphereGeometry } from "three";
import { mergeGeometries } from "three/examples/jsm/utils/BufferGeometryUtils.js";

import { createInkMaterial, createInkOutlineMaterial } from "@/ink/InkMaterial";

// The hull width is shader-driven now (screen-constant clip-space extrusion), so
// the mesh itself is unscaled.
const OUTLINE_SCALE = 1.0;

export default function InkDummy() {
  const geo = useMemo(() => {
    const head = new SphereGeometry(0.16, 24, 16);
    head.translate(0, 0.2, 0);
    const body = new CapsuleGeometry(0.1, 0.24, 8, 16);
    body.translate(0, -0.08, 0);
    const merged = mergeGeometries([head, body], false);
    head.dispose();
    body.dispose();
    return merged;
  }, []);

  const inkMat = useMemo(() => createInkMaterial(), []);
  const outlineMat = useMemo(() => createInkOutlineMaterial(), []);

  useEffect(
    () => () => {
      geo.dispose();
      inkMat.dispose();
      outlineMat.dispose();
    },
    [geo, inkMat, outlineMat],
  );

  return (
    <group position={[1.5, -1.55, -0.12]}>
      <mesh geometry={geo} material={outlineMat} scale={OUTLINE_SCALE} />
      <mesh geometry={geo} material={inkMat} />
    </group>
  );
}
