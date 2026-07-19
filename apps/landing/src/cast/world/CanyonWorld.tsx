"use client";

// The world root for M2. Owns the ONE camera + fog useFrame (default priority 0
// -- no numeric priority until the composer lands, which would disable R3F
// auto-render), reads the playhead via the store singleton every frame (never a
// hook selector), and gates visibility: the canyon renders while the playhead is
// in the scene 0-2 range, the kept placeholder markers render below it. The
// camera is one continuous pure-f(t) descent with a backward-fall framing across
// the canyon (looks up/along as the towers stream past). Nothing here allocates
// per frame -- all scratch objects are preallocated.

import { useMemo, useRef } from "react";
import { FogExp2, type Group, Vector3 } from "three";
import { useFrame } from "@react-three/fiber";

import { playhead, type QualityTier } from "@/engine/store";

import {
  cameraLookUpY,
  cameraSwayX,
  cameraY,
  cameraZ,
  canyonFogRGB,
} from "./canyon-layout";
import { CanyonScene } from "./CanyonScene";
import { PlaceholderMarkers } from "./PlaceholderMarkers";

export function CanyonWorld({ tier }: { tier: QualityTier }) {
  const canyonRef = useRef<Group>(null);
  const markersRef = useRef<Group>(null);

  // Preallocated scratch -- mutated in place each frame, never reallocated.
  const fog = useMemo(() => new FogExp2(0x0a0f18, 0.012), []);
  const target = useMemo(() => new Vector3(), []);
  const fogRGB = useMemo<[number, number, number]>(() => [0, 0, 0], []);

  useFrame((state) => {
    const t = playhead.t;

    // One continuous descent + canyon backward-fall framing (pure f(t)).
    const cam = state.camera;
    cam.position.set(cameraSwayX(t), cameraY(t), cameraZ(t));
    target.set(cameraSwayX(t) * 0.4, cameraY(t) + cameraLookUpY(t), -8);
    cam.lookAt(target);

    // Depth-graded haze (sodium-orange -> cold blue). Sharing fog.color as the
    // scene background blends distant towers into the haze for the depth read.
    canyonFogRGB(t, fogRGB);
    fog.color.setRGB(fogRGB[0], fogRGB[1], fogRGB[2]);
    state.scene.fog = fog;
    state.scene.background = fog.color;

    // Visibility gate: canyon for scenes 0-2, markers below (skips the other
    // group's draw calls entirely -> canyon segment stays within budget).
    const inCanyon = playhead.scene <= 2;
    if (canyonRef.current) canyonRef.current.visible = inCanyon;
    if (markersRef.current) markersRef.current.visible = !inCanyon;
  });

  return (
    <>
      {/* Baked night light rig (ADR-0016: "baked, crossfaded light states, not
          dynamic PBR lights") -- cheap stock three lights, no per-window point
          lights. Cool navy ambient floor + a cool-sky/warm-ground hemisphere
          (the "warm rim from windows, cool ambient" cast on the lit paper/tower
          materials) + a cool blue directional key for shape definition. */}
      <ambientLight color="#0d1626" intensity={0.35} />
      <hemisphereLight color="#2c3a52" groundColor="#3a2410" intensity={0.6} />
      <directionalLight color="#5b7fb5" position={[4, 8, 6]} intensity={0.4} />

      <group ref={canyonRef}>
        <CanyonScene tier={tier} />
      </group>

      <group ref={markersRef} visible={false}>
        <PlaceholderMarkers />
      </group>
    </>
  );
}
