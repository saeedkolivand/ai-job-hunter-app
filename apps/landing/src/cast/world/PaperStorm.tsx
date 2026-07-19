"use client";

// The paper storm: ONE InstancedMesh (one draw call) of thousands of falling
// rejection sheets. Every slot's base transform + shader seeds are initialized
// ONCE at build; the storm "thickens" through the canyon by revealing a leading
// subset via .count (a pure f(t) density ramp), and each sheet flutters via the
// analytic vertex shader driven by the playhead uniform -- no CPU per-sheet work,
// no per-frame allocation. Camera descent parallax makes the frozen cloud stream
// past. Animation useFrame stays at the DEFAULT priority (0).

import { useEffect, useMemo, useRef } from "react";
import {
  DoubleSide,
  Euler,
  InstancedBufferAttribute,
  type InstancedMesh,
  Matrix4,
  MeshLambertMaterial,
  PlaneGeometry,
  Quaternion,
  Vector3,
} from "three";
import { useFrame } from "@react-three/fiber";

import { stormCountForTier } from "@/engine/quality-ladder";
import { playhead, type QualityTier } from "@/engine/store";

import { stormActiveCount, stormInstance } from "./canyon-layout";
import { installStormShader } from "./shaders/paper-storm-shader";

export function PaperStorm({ tier }: { tier: QualityTier }) {
  const meshRef = useRef<InstancedMesh>(null);
  const count = stormCountForTier(tier);

  // Build geometry + material + per-instance seed attributes ONCE per tier.
  const { geometry, material, uniforms } = useMemo(() => {
    const geo = new PlaneGeometry(0.9, 1.2, 3, 1); // a few width segments so the bend reads
    const seeds = new Float32Array(count);
    const phases = new Float32Array(count);
    for (let i = 0; i < count; i++) {
      const inst = stormInstance(i);
      seeds[i] = inst.seed;
      phases[i] = inst.phase;
    }
    geo.setAttribute("aSeed", new InstancedBufferAttribute(seeds, 1));
    geo.setAttribute("aPhase", new InstancedBufferAttribute(phases, 1));
    // Lit (MeshLambert) so the fluttering sheets catch the canyon light; the
    // installer also attaches the procedural letter atlas as the material map.
    const mat = new MeshLambertMaterial({ color: 0xd8d2c4, side: DoubleSide, fog: true });
    const u = installStormShader(mat);
    return { geometry: geo, material: mat, uniforms: u };
  }, [count]);

  // Release the GPU resources this component owns when the tier changes / it
  // unmounts (the mesh element disposes its own attributes, but the geometry /
  // material we constructed outside JSX must be disposed by us).
  useEffect(() => {
    return () => {
      geometry.dispose();
      material.dispose();
    };
  }, [geometry, material]);

  // Initialize EVERY instance's base transform once the mesh exists. Setup-only
  // allocations (never per frame). No instanceColor is used, so no slot can flash
  // white; brightness rides the aSeed attribute in the shader.
  useEffect(() => {
    const mesh = meshRef.current;
    if (!mesh) return;
    const m = new Matrix4();
    const pos = new Vector3();
    const quat = new Quaternion();
    const euler = new Euler();
    const scl = new Vector3();
    for (let i = 0; i < count; i++) {
      const inst = stormInstance(i);
      pos.set(inst.x, inst.y, inst.z);
      euler.set(inst.rotX, inst.rotY, inst.rotZ);
      quat.setFromEuler(euler);
      scl.setScalar(inst.scale);
      m.compose(pos, quat, scl);
      mesh.setMatrixAt(i, m);
    }
    mesh.instanceMatrix.needsUpdate = true;
    mesh.count = 0; // hidden until the frame loop reveals via the density ramp
  }, [count]);

  useFrame(() => {
    const mesh = meshRef.current;
    if (!mesh) return;
    const t = playhead.t;
    uniforms.uStormT.value = t; // pure f(t) flutter -- reversible on rewind
    mesh.count = stormActiveCount(t, count); // thicken through the canyon
  });

  return <instancedMesh ref={meshRef} args={[geometry, material, count]} frustumCulled={false} />;
}
