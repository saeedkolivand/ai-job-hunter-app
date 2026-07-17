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
// advances the sketchbook boil clock via clocks.boilTime (stepped to the tier's
// boilFps), then (4) renders the composer.

import { EffectComposer, EffectPass, RenderPass } from "postprocessing";
import { useEffect, useMemo } from "react";
import { HalfFloatType, type Uniform } from "three";
import { useFrame, useThree } from "@react-three/fiber";

import { boilTime } from "@/engine/clocks";
import { evalPose } from "@/engine/journey";
import { resolveTier } from "@/engine/quality";
import { journeyStore } from "@/engine/store";
import { SketchbookEffect } from "@/post/sketchbook-effect";

export function Composer() {
  const gl = useThree((s) => s.gl);
  const scene = useThree((s) => s.scene);
  const camera = useThree((s) => s.camera);

  // Build the pipeline once. RenderPass draws the scene, then Pass A (the always
  // -on SketchbookEffect). Rebuilds only if the renderer/scene/camera identity
  // changes (it does not, in practice).
  const { composer, boilUniform, tier } = useMemo(() => {
    const tier = resolveTier();
    const composer = new EffectComposer(gl, {
      frameBufferType: HalfFloatType,
      multisampling: 0,
    });
    composer.addPass(new RenderPass(scene, camera));
    const sketchbook = new SketchbookEffect();
    composer.addPass(new EffectPass(camera, sketchbook));
    const boilUniform = sketchbook.uniforms.get("uBoilTime") as Uniform;
    return { composer, boilUniform, tier };
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
    boilUniform.value = boilTime(state.clock.elapsedTime, tier.boilFps);
    composer.render(delta);
  }, 1);

  return null;
}
