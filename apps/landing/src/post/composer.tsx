"use client";

// The one and only render driver for the GL experience. Lives inside the R3F
// <Canvas>. It owns the postprocessing EffectComposer (built once) and the sole
// priority>0 useFrame in the app: because a positive priority hands the render
// loop to us, R3F stops auto-rendering and every frame flows through
// composer.render() here.
//
// Per frame it: (1) reads the scroll-driven global t from journeyStore, (2)
// evaluates the camera pose for that t and copies it onto the default camera
// (evalPose writes into reusable objects -> zero per-frame allocation), (3)
// advances both boil clocks -- the sketchbook effect uniform and ink/boil's
// shared line-boil clock -- via clocks.boilTime (stepped to the tier's
// boilFps), then (4) renders the composer.

import {
  BloomEffect,
  ChromaticAberrationEffect,
  EffectComposer,
  EffectPass,
  RenderPass,
  ScanlineEffect,
} from "postprocessing";
import { useEffect, useMemo } from "react";
import { HalfFloatType, type Uniform, Vector2 } from "three";
import { useFrame, useThree } from "@react-three/fiber";

import { boilTime } from "@/engine/clocks";
import { evalPose } from "@/engine/journey";
import { resolveTier } from "@/engine/quality";
import { journeyStore } from "@/engine/store";
import { boilClock } from "@/ink/boil";
import { FriedEffect } from "@/post/fried-effect";
import { applyRecipe, type PassBHandles } from "@/post/recipes";
import { SketchbookEffect } from "@/post/sketchbook-effect";

export function Composer() {
  const gl = useThree((s) => s.gl);
  const scene = useThree((s) => s.scene);
  const camera = useThree((s) => s.camera);

  // Build the pipeline once. RenderPass draws the scene, then Pass A (the always
  // -on SketchbookEffect), then Pass B (the DEEP FRIED nuclear stack, ramped on
  // only inside the fried t-window by applyRecipe). Rebuilds only if the
  // renderer/scene/camera identity changes (it does not, in practice).
  const { composer, boilUniform, passB, tier } = useMemo(() => {
    const tier = resolveTier();
    const composer = new EffectComposer(gl, {
      frameBufferType: HalfFloatType,
      multisampling: 0,
    });
    composer.addPass(new RenderPass(scene, camera));
    const sketchbook = new SketchbookEffect();
    const passSketch = new EffectPass(camera, sketchbook);
    composer.addPass(passSketch);
    const boilUniform = sketchbook.uniforms.get("uBoilTime") as Uniform;

    // Pass B is TWO EffectPasses, not one. The library forbids a mainUv effect
    // (FriedEffect's barrel crunch transforms UV) from sharing a pass with any
    // EffectAttribute.CONVOLUTION effect, and ChromaticAberrationEffect IS the
    // convolution effect here (it resamples the input buffer at shifted UVs).
    // BloomEffect is NOT convolution -- its glow is precomputed into its own map
    // in update() and its mainImage just samples that -- so Bloom + CA merge
    // legally (a single convolution, no mainUv). FriedEffect + Scanline form the
    // second pass (no convolution there, so mainUv is allowed). Order preserves
    // the intended bloom -> CA -> fried -> scanline chain.
    const bloom = new BloomEffect({ mipmapBlur: true, luminanceThreshold: 0.6, intensity: 0 });
    const chromaticAberration = new ChromaticAberrationEffect({
      offset: new Vector2(0, 0),
      radialModulation: true,
      modulationOffset: 0.15,
    });
    const fried = new FriedEffect();
    const scanline = new ScanlineEffect({ density: 1.25 });
    const passBloom = new EffectPass(camera, bloom, chromaticAberration);
    const passFried = new EffectPass(camera, fried, scanline);
    passBloom.enabled = false;
    passFried.enabled = false;
    composer.addPass(passBloom);
    composer.addPass(passFried);

    // Final-output routing is NOT array-position based here. autoRenderToScreen
    // would pin renderToScreen onto the LAST pass (passFried) once, at addPass
    // time -- but passFried is disabled outside the fried window, so the composite
    // would never reach the screen for any other beat (the whole journey rendered
    // black except deep-fried). We drive renderToScreen per frame in applyRecipe
    // instead: the last ENABLED pass presents. Outside fried that is passSketch;
    // inside it, passFried.
    composer.autoRenderToScreen = false;
    passSketch.renderToScreen = true;

    const passB: PassBHandles = {
      passSketch,
      passBloom,
      passFried,
      bloom,
      chromaticAberration,
      fried,
      scanline,
    };
    return { composer, boilUniform, passB, tier };
  }, [gl, scene, camera]);

  // Keep the composer's buffers matched to the viewport. setSize takes CSS pixels
  // and applies the renderer pixel ratio internally.
  useEffect(() => {
    const onResize = () => composer.setSize(window.innerWidth, window.innerHeight);
    onResize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [composer]);

  // Release GPU resources when the Canvas unmounts.
  useEffect(() => () => composer.dispose(), [composer]);

  useFrame((state, delta) => {
    const t = journeyStore.getState().t;
    const pose = evalPose(t);
    state.camera.position.copy(pose.position);
    state.camera.quaternion.copy(pose.quaternion);
    // Single writer for BOTH boil clocks: the sketchbook effect's uniform and
    // the shared line-boil clock every InkStrokes material references
    // (ink/boil.ts). Priority 1 runs after all priority-0 subscribers and
    // immediately before composer.render() -- the only consumer -- so ordering
    // holds and the whole page steps on one value per frame.
    const bt = boilTime(state.clock.elapsedTime, tier.boilFps);
    boilUniform.value = bt;
    boilClock.value = bt;
    // Pass B recipe: pure f(t) -> all fried uniforms + both pass-enabled flags.
    // Runs here (priority 1, after all priority-0 subscribers, right before the
    // only consumer) so the whole page steps on one t per frame.
    applyRecipe(t, passB);
    composer.render(delta);
  }, 1);

  return null;
}
