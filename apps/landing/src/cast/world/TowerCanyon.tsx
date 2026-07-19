"use client";

// The rejection-tower canyon: ONE InstancedMesh (one draw call) of tall boxes in
// two walls the camera falls between. Every instance's transform + per-tower seed
// is set ONCE; the emissive window grid is procedural in the fragment shader (no
// texture, per the <=10 MB budget). The only per-frame work is mutating the
// uTowerT uniform from the playhead so a few windows slowly flicker -- a single
// uniform write, no allocation, default useFrame priority (0).

import { useEffect, useMemo, useRef } from "react";
import {
  BoxGeometry,
  InstancedBufferAttribute,
  type InstancedMesh,
  Matrix4,
  MeshStandardMaterial,
  Quaternion,
  Vector3,
} from "three";
import { useFrame } from "@react-three/fiber";

import { towerCountForTier } from "@/engine/quality-ladder";
import { playhead, type QualityTier } from "@/engine/store";

import { towerInstance } from "./canyon-layout";
import { installTowerShader } from "./shaders/tower-window-shader";

export function TowerCanyon({ tier }: { tier: QualityTier }) {
  const meshRef = useRef<InstancedMesh>(null);
  const count = towerCountForTier(tier);

  const { geometry, material, uniforms } = useMemo(() => {
    const geo = new BoxGeometry(1, 1, 1); // unit box, scaled per instance
    const seeds = new Float32Array(count);
    for (let i = 0; i < count; i++) seeds[i] = towerInstance(i).seed;
    geo.setAttribute("aTowerSeed", new InstancedBufferAttribute(seeds, 1));
    const mat = new MeshStandardMaterial({ color: 0x14161d, roughness: 0.92, metalness: 0 });
    const u = installTowerShader(mat);
    return { geometry: geo, material: mat, uniforms: u };
  }, [count]);

  useEffect(() => {
    return () => {
      geometry.dispose();
      material.dispose();
    };
  }, [geometry, material]);

  useEffect(() => {
    const mesh = meshRef.current;
    if (!mesh) return;
    const m = new Matrix4();
    const pos = new Vector3();
    const scl = new Vector3();
    const quat = new Quaternion(); // identity -- towers are axis-aligned
    for (let i = 0; i < count; i++) {
      const t = towerInstance(i);
      pos.set(t.x, t.y, t.z);
      scl.set(t.w, t.h, t.d);
      m.compose(pos, quat, scl);
      mesh.setMatrixAt(i, m);
    }
    mesh.instanceMatrix.needsUpdate = true;
  }, [count]);

  useFrame(() => {
    uniforms.uTowerT.value = playhead.t; // pure f(t) flicker -- reversible on rewind
  });

  return <instancedMesh ref={meshRef} args={[geometry, material, count]} frustumCulled={false} />;
}
