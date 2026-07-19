"use client";

// The world root (scenes 0-4 as of M3). Owns the ONE camera + fog + exposure
// useFrame (default priority 0 -- no numeric priority until the composer lands,
// which would disable R3F auto-render), reads the playhead via the store
// singleton every frame (never a hook selector), and gates each scene's group by
// the pure worldLayers() visibility map. The camera is one continuous pure-f(t)
// descent: canyon backward-fall framing (scenes 0-2), then looking down through
// the surface into the deep (scenes 2-4). Nothing here allocates per frame -- all
// scratch objects are preallocated.
//
// The single real-time smoothing in the render path lives here: the luminance-
// velocity clamp (LuminanceClamp) eases the APPLIED tone-mapping exposure toward
// the pure-f(t) target (sceneLuminance) so fast scrubbing cannot strobe the
// blackout transitions (WCAG 2.3.1; see engine/luminance-clamp.ts). At rest it
// converges to the target, so determinism holds.

import { useMemo, useRef } from "react";
import { FogExp2, type Group, type PointLight, Vector3 } from "three";
import { useFrame } from "@react-three/fiber";

import { LuminanceClamp } from "@/engine/luminance-clamp";
import { playhead, type QualityTier } from "@/engine/store";

import { cameraLookUpY, cameraSwayX, cameraY, cameraZ } from "./canyon-layout";
import { CanyonScene } from "./CanyonScene";
import { DeepScene } from "./DeepScene";
import { PlaceholderMarkers } from "./PlaceholderMarkers";
import { SplashCrown } from "./SplashCrown";
import {
  amberIntensity,
  amberLightY,
  cameraLookDownOffset,
  MAX_LUMINANCE_SLEW_PER_SEC,
  sceneLuminance,
  worldFog,
  type WorldLayers,
  writeWorldLayers,
} from "./water-layout";
import { WaterSurface } from "./WaterSurface";

export function CanyonWorld({ tier }: { tier: QualityTier }) {
  const canyonRef = useRef<Group>(null);
  const waterRef = useRef<Group>(null);
  const splashRef = useRef<Group>(null);
  const deepRef = useRef<Group>(null);
  const markersRef = useRef<Group>(null);
  const amberRef = useRef<PointLight>(null);

  // Preallocated scratch -- mutated in place each frame, never reallocated.
  const fog = useMemo(() => new FogExp2(0x0a0f18, 0.012), []);
  const target = useMemo(() => new Vector3(), []);
  const fogRGB = useMemo<[number, number, number]>(() => [0, 0, 0], []);
  const layers = useMemo<WorldLayers>(
    () => ({ canyon: false, water: false, splash: false, deep: false, markers: false }),
    [],
  );
  // The ONE real-time (delta-driven) smoothing in the render path -- the WCAG
  // luminance clamp. The constructor's `initial` argument is a throwaway: the
  // class hard-cuts to the ACTUAL scene target on its very first step() call
  // (see engine/luminance-clamp.ts "MOUNT PRIMING"), so a (re)mount landing
  // anywhere in the dark scenes -- a hash deep-link, or the reduce->restore GL
  // remount -- renders the correct target immediately instead of visibly fading
  // in from this constant.
  const luminance = useMemo(() => new LuminanceClamp(1, MAX_LUMINANCE_SLEW_PER_SEC), []);

  useFrame((state, delta) => {
    const t = playhead.t;

    // One continuous descent: canyon backward-fall framing up top, looking down
    // through the surface into the deep below (both pure f(t), and the two
    // framings never overlap -- canyon look-up is 0 by the surface).
    const cam = state.camera;
    cam.position.set(cameraSwayX(t), cameraY(t), cameraZ(t));
    target.set(
      cameraSwayX(t) * 0.4,
      cameraY(t) + cameraLookUpY(t) + cameraLookDownOffset(t),
      -8,
    );
    cam.lookAt(target);

    // Depth-graded haze for the whole descent: sodium/blue canyon night up top,
    // midnight blue-black closing in for the deep + blackout below.
    const density = worldFog(t, fogRGB);
    fog.color.setRGB(fogRGB[0], fogRGB[1], fogRGB[2]);
    fog.density = density;
    state.scene.fog = fog;
    state.scene.background = fog.color;

    // Luminance-velocity clamp: ease the APPLIED exposure toward the pure-f(t)
    // target, rate-limited per REAL second so a fast scrub cannot strobe the
    // blackout transitions (the ONE render-path real-time smoothing exception).
    state.gl.toneMappingExposure = luminance.step(sceneLuminance(t), delta);

    // The single warm amber point appearing below and growing in the blackout
    // (pure f(t); slow, so no clamp needed).
    const amber = amberRef.current;
    if (amber) {
      amber.intensity = amberIntensity(t);
      amber.position.set(0, amberLightY(t), -2);
    }

    // Visibility gate: only the active scene's group draws (keeps each segment
    // within the draw-call budget). Writes into the preallocated `layers` scratch
    // -- no fresh object per frame.
    writeWorldLayers(playhead.scene, layers);
    if (canyonRef.current) canyonRef.current.visible = layers.canyon;
    if (waterRef.current) waterRef.current.visible = layers.water;
    if (splashRef.current) splashRef.current.visible = layers.splash;
    if (deepRef.current) deepRef.current.visible = layers.deep;
    if (markersRef.current) markersRef.current.visible = layers.markers;
  });

  return (
    <>
      {/* Baked night light rig (ADR-0016: "baked, crossfaded light states, not
          dynamic PBR lights") -- cool navy ambient floor + a cool-sky/warm-ground
          hemisphere + a cool blue directional key. The deep dims via the exposure
          clamp above (not by touching these), so the canyon look is unchanged. */}
      <ambientLight color="#0d1626" intensity={0.35} />
      <hemisphereLight color="#2c3a52" groundColor="#3a2410" intensity={0.6} />
      <directionalLight color="#5b7fb5" position={[4, 8, 6]} intensity={0.4} />
      {/* The blackout amber point -- position + intensity driven per frame. */}
      <pointLight ref={amberRef} color="#ff8a3c" intensity={0} distance={150} decay={1.3} />

      <group ref={canyonRef}>
        <CanyonScene tier={tier} />
      </group>

      <group ref={waterRef} visible={false}>
        <WaterSurface tier={tier} />
      </group>

      <group ref={splashRef} visible={false}>
        <SplashCrown />
      </group>

      <group ref={deepRef} visible={false}>
        <DeepScene tier={tier} />
      </group>

      <group ref={markersRef} visible={false}>
        <PlaceholderMarkers />
      </group>
    </>
  );
}
