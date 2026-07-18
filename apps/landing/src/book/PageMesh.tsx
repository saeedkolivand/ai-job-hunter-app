"use client";

// The one M1 page: the pre-split corner-tear kraft page. Renders the two
// geometries from presplitCornerTear -- the HELD piece static on the notebook,
// the FREE corner piece inside a group the composer throws (rigid, f(exitP)).
// Both meshes share ONE RipPaperMaterial program (distinguished only by a uSide
// flag): the vertex shader gates the tear front along aSeamArc and bends the
// peel near the seam, all pure f(uRipP) so scrubbing back reassembles the page;
// the fragment is the placeholder kraft (real bake lands in M2). The split and
// the two material instances are built once (pure f(seed)) and disposed on
// unmount.

import { type RefObject, useEffect, useMemo } from "react";
import type { Group } from "three";

import { createRipPaperMaterial, RIP_PAPER_SIDE } from "@/materials/RipPaperMaterial";
import { presplitCornerTear } from "@/rip/presplit";

export default function PageMesh({ freeRef }: { freeRef: RefObject<Group | null> }) {
  const split = useMemo(() => presplitCornerTear(1), []);
  const heldMat = useMemo(() => createRipPaperMaterial(RIP_PAPER_SIDE.HELD), []);
  const freeMat = useMemo(() => createRipPaperMaterial(RIP_PAPER_SIDE.FREE), []);

  useEffect(
    () => () => {
      split.held.dispose();
      split.free.dispose();
      heldMat.dispose();
      freeMat.dispose();
    },
    [split, heldMat, freeMat],
  );

  return (
    <group>
      <mesh geometry={split.held} material={heldMat} />
      <group ref={freeRef}>
        <mesh geometry={split.free} material={freeMat} />
      </group>
    </group>
  );
}
