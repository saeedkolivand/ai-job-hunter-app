"use client";

// The splash crown (M3, scene 2 [0.30, 0.38)): the full in-house VAT playback
// path, playing a PROCEDURAL stand-in bake so the real Houdini FLIP crown is a
// drop-in later (DCC ownership is an open ADR-0016 item). At load it bakes an
// analytic crown (splash-crown.ts) into a small float DataTexture -- 64 frames x
// the ring vertex count -- then plays it back via the in-house VAT decode shader.
// The frame pair + blend are the deterministic, unit-tested engine/vat.ts math
// keyed off scene-2 progress, so the splash is scrub-reversible both directions.
// Animation useFrame stays at the DEFAULT priority (0). The bake runs in useMemo,
// which only executes client-side (the crown mounts under gl-live behind the
// Experience gate) and touches no window/document, so it is SSR-safe.

import { useEffect, useMemo } from "react";
import {
  BufferAttribute,
  DataTexture,
  DoubleSide,
  FloatType,
  MeshStandardMaterial,
  NearestFilter,
  RGBAFormat,
  RingGeometry,
} from "three";
import { useFrame } from "@react-three/fiber";

import { sceneProgress } from "@/engine/scene-resolver";
import { playhead } from "@/engine/store";
import { frameRowV, type VatFrameSample, type VatMeta, vatProgress, writeVatFrameIndex } from "@/engine/vat";

import { installVatCrownShader } from "./shaders/vat-crown-shader";
import {
  CROWN_FRAMES,
  CROWN_RADIUS,
  CROWN_RINGS,
  CROWN_SEGMENTS,
  crownVertexAt,
  normalizeAngle,
} from "./splash-crown";
import { SURFACE_WORLD_Y } from "./water-layout";

export function SplashCrown() {
  const { geometry, material, uniforms, meta } = useMemo(() => {
    // Rest mesh: a radial disc of vertices (RingGeometry lays vertices out ring by
    // ring). The bake writes one absolute crown position per (frame, vertex).
    const geo = new RingGeometry(0.02, CROWN_RADIUS, CROWN_SEGMENTS, CROWN_RINGS);
    const posAttr = geo.getAttribute("position"); // non-optional accessor
    const vertices = posAttr.count;

    const ids = new Float32Array(vertices);
    for (let i = 0; i < vertices; i++) ids[i] = i;
    geo.setAttribute("aVatId", new BufferAttribute(ids, 1)); // per-vertex texture column

    // Bake: row = frame, column = vertex, RGB = baked position.
    const data = new Float32Array(CROWN_FRAMES * vertices * 4);
    const out: [number, number, number] = [0, 0, 0];
    for (let fr = 0; fr < CROWN_FRAMES; fr++) {
      for (let v = 0; v < vertices; v++) {
        const rx = posAttr.getX(v);
        const ry = posAttr.getY(v); // RingGeometry lies in the XY plane
        const radius = Math.hypot(rx, ry);
        const angle = normalizeAngle(Math.atan2(ry, rx));
        crownVertexAt(radius, angle, fr, CROWN_FRAMES, out);
        const idx = (fr * vertices + v) * 4;
        data[idx] = out[0];
        data[idx + 1] = out[1];
        data[idx + 2] = out[2];
        data[idx + 3] = 1;
      }
    }
    const tex = new DataTexture(data, vertices, CROWN_FRAMES, RGBAFormat, FloatType);
    tex.minFilter = NearestFilter; // exact texel per column; row lerp is done on the CPU (uBlend)
    tex.magFilter = NearestFilter;
    tex.generateMipmaps = false;
    tex.needsUpdate = true;

    const mat = new MeshStandardMaterial({
      color: 0xbcccd6, // pale foam
      emissive: 0x0e161e,
      roughness: 0.3,
      metalness: 0,
      transparent: true,
      opacity: 0.85,
      side: DoubleSide, // displaced disc -- render both faces so the crown never gaps
    });
    const u = installVatCrownShader(mat, tex, vertices);
    const m: VatMeta = { frames: CROWN_FRAMES, vertices, duration: 0.85 };
    return { geometry: geo, material: mat, uniforms: u, meta: m };
  }, []);

  useEffect(() => {
    const tex = uniforms.uVatTex.value as { dispose?: () => void } | null;
    return () => {
      geometry.dispose();
      material.dispose();
      tex?.dispose?.();
    };
  }, [geometry, material, uniforms]);

  // Preallocated scratch -- writeVatFrameIndex mutates it in place each frame,
  // never reallocated.
  const sample = useMemo<VatFrameSample>(() => ({ a: 0, b: 0, blend: 0 }), []);

  useFrame(() => {
    const p = vatProgress(sceneProgress(playhead.t, 2), meta);
    writeVatFrameIndex(p, meta.frames, sample);
    uniforms.uRowA.value = frameRowV(sample.a, meta.frames);
    uniforms.uRowB.value = frameRowV(sample.b, meta.frames);
    uniforms.uBlend.value = sample.blend; // pure f(t) -- reversible on rewind
  });

  return (
    <mesh
      geometry={geometry}
      material={material}
      position={[0, SURFACE_WORLD_Y, -4]}
      frustumCulled={false}
    />
  );
}
