"use client";

// The paper-ocean surface (M3): a bounded Gerstner water patch seen from above
// during the fall approach (scene 2) and from below in the deep (scene 3), paved
// with a sparse instanced layer of floating rejection letters. The water plane's
// swell is analytic in the vertex shader (installWaterShader, pure f(t)); the
// floating letters ride the SAME Gerstner surface (water-layout.gerstnerSurface,
// the shared source of truth) with per-frame matrices, and reuse the storm shader
// for the letter atlas + a wet-paper micro-flutter. One draw call for the water,
// one for the letters. Animation useFrame stays at the DEFAULT priority (0).

import { useEffect, useMemo, useRef } from "react";
import {
  DoubleSide,
  InstancedBufferAttribute,
  type InstancedMesh,
  Matrix4,
  MeshLambertMaterial,
  MeshStandardMaterial,
  PlaneGeometry,
  Quaternion,
  Vector3,
} from "three";
import { useFrame } from "@react-three/fiber";

import { surfaceLetterCountForTier, waterSegmentsForTier } from "@/engine/quality-ladder";
import { playhead, type QualityTier } from "@/engine/store";

import { installStormShader } from "./shaders/paper-storm-shader";
import { installWaterShader } from "./shaders/water-surface-shader";
import {
  gerstnerSurface,
  SURFACE_PATCH,
  SURFACE_WORLD_Y,
  type SurfaceLetter,
  surfaceLetterInstance,
  type SurfacePoint,
} from "./water-layout";

const UNIT_Z = new Vector3(0, 0, 1);

export function WaterSurface({ tier }: { tier: QualityTier }) {
  const lettersRef = useRef<InstancedMesh>(null);
  const seg = waterSegmentsForTier(tier);
  const letterCount = surfaceLetterCountForTier(tier);

  // Water plane: rotate the GEOMETRY (not the mesh) into the XZ plane so the
  // shader's y-up Gerstner displacement works in object space directly.
  const { waterGeo, waterMat, waterUniforms } = useMemo(() => {
    const geo = new PlaneGeometry(SURFACE_PATCH, SURFACE_PATCH, seg, seg);
    geo.rotateX(-Math.PI / 2);
    const mat = new MeshStandardMaterial({
      color: 0x05080e, // dark midnight water (ADR palette)
      roughness: 0.14,
      metalness: 0,
      side: DoubleSide,
      fog: true,
    });
    const u = installWaterShader(mat);
    return { waterGeo: geo, waterMat: mat, waterUniforms: u };
  }, [seg]);

  // Floating letters: NOT rotated (faces object +Z), so the reused storm shader's
  // object-space flutter curls the sheet along its own normal. aSeed/aPhase drive
  // the atlas cell + flutter, exactly like the storm.
  const { letterGeo, letterMat, letterUniforms } = useMemo(() => {
    const geo = new PlaneGeometry(0.9, 1.2, 3, 1);
    const seeds = new Float32Array(letterCount);
    const phases = new Float32Array(letterCount);
    for (let i = 0; i < letterCount; i++) {
      const inst = surfaceLetterInstance(i);
      seeds[i] = inst.seed;
      phases[i] = inst.phase;
    }
    geo.setAttribute("aSeed", new InstancedBufferAttribute(seeds, 1));
    geo.setAttribute("aPhase", new InstancedBufferAttribute(phases, 1));
    const mat = new MeshLambertMaterial({ color: 0x8f8a7e, side: DoubleSide, fog: true });
    const u = installStormShader(mat);
    return { letterGeo: geo, letterMat: mat, letterUniforms: u };
  }, [letterCount]);

  useEffect(() => {
    return () => {
      waterGeo.dispose();
      waterMat.dispose();
      letterGeo.dispose();
      letterMat.dispose();
    };
  }, [waterGeo, waterMat, letterGeo, letterMat]);

  // Preallocated scratch -- mutated in place each frame, never reallocated.
  const scratch = useMemo(
    () => ({
      m: new Matrix4(),
      pos: new Vector3(),
      quat: new Quaternion(),
      yaw: new Quaternion(),
      normal: new Vector3(),
      scl: new Vector3(),
      pt: { x: 0, y: 0, z: 0, nx: 0, ny: 1, nz: 0 } as SurfacePoint,
      letters: [] as SurfaceLetter[],
    }),
    [],
  );

  // Initialize every letter instance once (no unset/white slot), then let the
  // frame loop ride them on the swell while the surface is on-screen.
  useEffect(() => {
    const mesh = lettersRef.current;
    if (!mesh) return;
    scratch.letters = Array.from({ length: letterCount }, (_, i) => surfaceLetterInstance(i));
    for (let i = 0; i < letterCount; i++) {
      const l = scratch.letters[i];
      if (!l) continue;
      scratch.pos.set(l.x, SURFACE_WORLD_Y, l.z);
      scratch.scl.setScalar(l.scale);
      scratch.m.compose(scratch.pos, scratch.quat.identity(), scratch.scl);
      mesh.setMatrixAt(i, scratch.m);
    }
    mesh.instanceMatrix.needsUpdate = true;
  }, [letterCount, scratch]);

  useFrame(() => {
    const t = playhead.t;
    waterUniforms.uWaterT.value = t; // pure f(t) swell -- reversible on rewind
    letterUniforms.uStormT.value = t; // pure f(t) wet-paper micro-flutter on the letters

    // Only re-ride the letters while the surface is on-screen (scene 2 or 3) --
    // outside that the group is hidden, so skip the per-letter matrix work.
    const active = playhead.scene === 2 || playhead.scene === 3;
    const mesh = lettersRef.current;
    if (!active || !mesh) return;
    const letters = scratch.letters;
    for (let i = 0; i < letters.length; i++) {
      const l = letters[i];
      if (!l) continue;
      gerstnerSurface(l.x, l.z, t, scratch.pt);
      // gerstnerSurface returns the already-displaced world x/z (rest + horizontal
      // Gerstner swirl); y is the height above the surface plane.
      scratch.pos.set(scratch.pt.x, SURFACE_WORLD_Y + scratch.pt.y, scratch.pt.z);
      scratch.normal.set(scratch.pt.nx, scratch.pt.ny, scratch.pt.nz);
      scratch.quat.setFromUnitVectors(UNIT_Z, scratch.normal);
      scratch.yaw.setFromAxisAngle(UNIT_Z, l.yaw);
      scratch.quat.multiply(scratch.yaw); // spin about the paper face, then align to the normal
      scratch.scl.setScalar(l.scale);
      scratch.m.compose(scratch.pos, scratch.quat, scratch.scl);
      mesh.setMatrixAt(i, scratch.m);
    }
    mesh.instanceMatrix.needsUpdate = true;
  });

  return (
    <>
      <mesh geometry={waterGeo} material={waterMat} position={[0, SURFACE_WORLD_Y, 0]} frustumCulled={false} />
      <instancedMesh ref={lettersRef} args={[letterGeo, letterMat, letterCount]} frustumCulled={false} />
    </>
  );
}
