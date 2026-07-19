"use client";

// The deep (M3, scenes 3-4 [0.38, 0.58)): the underwater volume. Depth-graded
// blue-black fog + the exposure dimming are owned by CanyonWorld; this file
// composes the volume contents:
//   - GodrayShafts: a handful of additive cone shafts thinning band by band into
//     the deep (the M3 godrays FALLBACK path -- no post composer; see the handoff).
//   - DeepPapers: a sparse frozen cloud of sinking sheets the camera drifts
//     through (reuses the storm InstancedMesh + shader pattern, one draw call).
//   - LimpFigure: a procedural capsule-body silhouette going limp as it sinks
//     (placeholder; the authored glTF clip is a later milestone).
// Everything is pure f(t). Animation useFrames stay at the DEFAULT priority (0).

import { useEffect, useMemo, useRef } from "react";
import {
  AdditiveBlending,
  Color,
  ConeGeometry,
  DoubleSide,
  Euler,
  type Group,
  InstancedBufferAttribute,
  type InstancedMesh,
  Matrix4,
  MeshBasicMaterial,
  MeshLambertMaterial,
  MeshStandardMaterial,
  PlaneGeometry,
  Quaternion,
  Vector3,
} from "three";
import { useFrame } from "@react-three/fiber";

import { deepPaperCountForTier, godrayShaftCountForTier } from "@/engine/quality-ladder";
import { playhead, type QualityTier } from "@/engine/store";

import { installGodrayShaftShader } from "./shaders/godray-shaft-shader";
import { installStormShader } from "./shaders/paper-storm-shader";
import {
  deepPaperInstance,
  type FigureState,
  godrayShaft,
  godrayStrength,
  SURFACE_WORLD_Y,
  writeLimpFigure,
} from "./water-layout";

// ---- sparse drifting papers (reuses the storm instancing + shader) ----------

function DeepPapers({ tier }: { tier: QualityTier }) {
  const meshRef = useRef<InstancedMesh>(null);
  const count = deepPaperCountForTier(tier);

  const { geometry, material, uniforms } = useMemo(() => {
    const geo = new PlaneGeometry(0.8, 1.1, 2, 1);
    const seeds = new Float32Array(count);
    const phases = new Float32Array(count);
    for (let i = 0; i < count; i++) {
      const inst = deepPaperInstance(i);
      seeds[i] = inst.seed;
      phases[i] = inst.phase;
    }
    geo.setAttribute("aSeed", new InstancedBufferAttribute(seeds, 1));
    geo.setAttribute("aPhase", new InstancedBufferAttribute(phases, 1));
    const mat = new MeshLambertMaterial({ color: 0x6b6a63, side: DoubleSide, fog: true });
    const u = installStormShader(mat);
    return { geometry: geo, material: mat, uniforms: u };
  }, [count]);

  useEffect(() => {
    return () => {
      geometry.dispose();
      material.dispose();
    };
  }, [geometry, material]);

  // Initialize every instance once (no unset/white slot).
  useEffect(() => {
    const mesh = meshRef.current;
    if (!mesh) return;
    const m = new Matrix4();
    const pos = new Vector3();
    const quat = new Quaternion();
    const euler = new Euler();
    const scl = new Vector3();
    for (let i = 0; i < count; i++) {
      const inst = deepPaperInstance(i);
      pos.set(inst.x, inst.y, inst.z);
      euler.set(inst.rotX, inst.rotY, inst.rotZ);
      quat.setFromEuler(euler);
      scl.setScalar(inst.scale);
      m.compose(pos, quat, scl);
      mesh.setMatrixAt(i, m);
    }
    mesh.instanceMatrix.needsUpdate = true;
  }, [count]);

  useFrame(() => {
    uniforms.uStormT.value = playhead.t; // slow flutter -- pure f(t), reversible
  });

  return <instancedMesh ref={meshRef} args={[geometry, material, count]} frustumCulled={false} />;
}

// ---- limp figure silhouette (procedural capsules; no character work) --------

function LimpFigure() {
  const groupRef = useRef<Group>(null);
  const scratch = useMemo<FigureState>(
    () => ({ x: 0, y: 0, z: 0, rx: 0, ry: 0, rz: 0, visible: false }),
    [],
  );
  const bodyMat = useMemo(
    () => new MeshStandardMaterial({ color: 0x0a0e14, roughness: 0.95, metalness: 0 }),
    [],
  );
  useEffect(() => {
    return () => bodyMat.dispose();
  }, [bodyMat]);

  useFrame(() => {
    const g = groupRef.current;
    if (!g) return;
    writeLimpFigure(playhead.t, scratch);
    g.visible = scratch.visible;
    if (!scratch.visible) return;
    g.position.set(scratch.x, scratch.y, scratch.z);
    g.rotation.set(scratch.rx, scratch.ry, scratch.rz);
  });

  // A loose, face-up-ish limp pose -- torso + head + splayed limbs. Silhouette
  // only; art comes later.
  return (
    <group ref={groupRef} visible={false}>
      <mesh material={bodyMat} rotation={[0, 0, 0.12]}>
        <capsuleGeometry args={[0.32, 1.0, 4, 8]} />
      </mesh>
      <mesh material={bodyMat} position={[0, 0.86, 0]}>
        <sphereGeometry args={[0.26, 16, 12]} />
      </mesh>
      <mesh material={bodyMat} position={[-0.45, 0.3, 0.05]} rotation={[0, 0, 1.15]}>
        <capsuleGeometry args={[0.12, 0.8, 4, 6]} />
      </mesh>
      <mesh material={bodyMat} position={[0.5, 0.28, -0.05]} rotation={[0, 0, -1.32]}>
        <capsuleGeometry args={[0.12, 0.8, 4, 6]} />
      </mesh>
      <mesh material={bodyMat} position={[-0.22, -0.95, 0.04]} rotation={[0.2, 0, 0.22]}>
        <capsuleGeometry args={[0.15, 0.95, 4, 6]} />
      </mesh>
      <mesh material={bodyMat} position={[0.24, -0.95, -0.03]} rotation={[-0.15, 0, -0.16]}>
        <capsuleGeometry args={[0.15, 0.95, 4, 6]} />
      </mesh>
    </group>
  );
}

// ---- additive god-ray shafts (the M3 fallback path) -------------------------

// Below this strength the additive cones are visually gone (uStrength alone
// already zeroes their alpha), but the fragments still rasterize -- gate the
// whole group's `visible` off below the threshold so the draw calls (and the
// additive overdraw) actually disappear through most of the blackout, not just
// the color.
const GODRAY_VISIBLE_THRESHOLD = 0.01;

function GodrayShafts({ tier }: { tier: QualityTier }) {
  const groupRef = useRef<Group>(null);
  const count = godrayShaftCountForTier(tier);

  // The cone geometry + additive material are count-INDEPENDENT (each shaft is
  // the same unit cone, scaled per instance via mesh props), so they are built
  // once; the shaft count only drives how many meshes are rendered.
  const { geometry, material, uniforms } = useMemo(() => {
    const geo = new ConeGeometry(1, 1, 14, 1, true); // unit cone, scaled per shaft
    const mat = new MeshBasicMaterial({
      color: 0xffffff,
      transparent: true,
      depthWrite: false,
      blending: AdditiveBlending,
      side: DoubleSide,
      fog: false,
    });
    // Shaft color darkened (M3 review fix): the old pale 0x9fc4d8 was bright
    // enough on its own that additive stacking of several shafts overexposed the
    // deep; a dimmer, less saturated blue keeps the "light shaft" hue read
    // without contributing so much raw luminance per shaft.
    const u = installGodrayShaftShader(mat, new Color(0x4a6478));
    return { geometry: geo, material: mat, uniforms: u };
  }, []);

  useEffect(() => {
    return () => {
      geometry.dispose();
      material.dispose();
    };
  }, [geometry, material]);

  useFrame(() => {
    const strength = godrayStrength(playhead.t); // thins with depth -- pure f(t)
    uniforms.uStrength.value = strength;
    // Gate the whole group off once the shafts are visually gone -- kills the
    // draw calls (and the additive overdraw) through most of the blackout,
    // rather than leaving 7 near-zero-alpha cones still rasterizing every frame.
    const g = groupRef.current;
    if (g) g.visible = strength > GODRAY_VISIBLE_THRESHOLD;
  });

  // A handful of additive cones (count is small -- these ARE the shaft draw
  // calls). Apex near the surface, widening down into the deep; placed statically
  // per shaft. dispose={null} so R3F never disposes the shared geo/material when a
  // single cone unmounts -- the effect above owns their disposal.
  return (
    <group ref={groupRef} visible={false}>
      {Array.from({ length: count }, (_, i) => {
        const s = godrayShaft(i);
        return (
          <mesh
            key={i}
            geometry={geometry}
            material={material}
            position={[s.x, SURFACE_WORLD_Y - s.len * 0.5 + 4, s.z]}
            rotation={[s.tiltX, 0, s.tiltZ]}
            scale={[s.radius, s.len, s.radius]}
            frustumCulled={false}
            dispose={null}
          />
        );
      })}
    </group>
  );
}

export function DeepScene({ tier }: { tier: QualityTier }) {
  return (
    <>
      <GodrayShafts tier={tier} />
      <DeepPapers tier={tier} />
      <LimpFigure />
    </>
  );
}
