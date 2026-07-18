// The scroll rig. Lenis owns document scroll, its rAF driven from gsap.ticker
// (single clock). ONE master GSAP ScrollTrigger, scrub:true timeline maps
// document scroll -> a raw target, and the playhead is the damped follow of that
// target -- written into the preallocated store object, never React state. The
// damping is the ONE skill-sanctioned smoothing; every derived visual then reads
// the playhead as a pure function of t.
//
// Client-only: nothing here runs at module scope. gsap.registerPlugin and all
// window access happen inside createScrollRig(), which the Experience component
// calls from an effect after the gate passes.

import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import Lenis from "lenis";

import { SCRUB } from "./constants";
import { resolveScene, sceneProgress } from "./scene-resolver";
import { playhead, useRig } from "./store";

let registered = false;
function ensureRegistered(): void {
  if (!registered) {
    gsap.registerPlugin(ScrollTrigger);
    registered = true;
  }
}

function clamp01(t: number): number {
  return t < 0 ? 0 : t > 1 ? 1 : t;
}

export interface ScrollRig {
  start: () => void;
  stop: () => void;
  destroy: () => void;
  // Reseed the playhead + scroll position to an exact t (chapter start / hash).
  seek: (t: number) => void;
}

export function createScrollRig(): ScrollRig {
  ensureRegistered();

  // scrub:true keeps `proxy.t` locked 1:1 to the scrollbar (the raw target);
  // absolute tween only -- no += accumulation, so a given scroll position always
  // produces one deterministic target.
  const proxy = { t: 0 };
  const lenis = new Lenis({ lerp: 0.1, wheelMultiplier: 1, touchMultiplier: 1 });

  const tween = gsap.to(proxy, {
    t: 1,
    ease: "none",
    scrollTrigger: {
      start: 0,
      end: "max",
      scrub: true,
      invalidateOnRefresh: true,
    },
  });

  lenis.on("scroll", ScrollTrigger.update);

  let last = 0;
  const onTick = (time: number): void => {
    lenis.raf(time * 1000);
    const now = time; // gsap.ticker time is in seconds
    const dt = last === 0 ? 1 / 60 : Math.min(0.1, Math.max(0.001, now - last));
    last = now;

    // Frame-rate independent exponential smoothing toward the raw scroll target.
    // At rest playhead.t converges exactly to proxy.t (== pure f(scroll)), so
    // scrolling down then rewinding to the same position yields the same frame.
    const k = 1 - Math.pow(1 - SCRUB, dt * 60);
    const prev = playhead.t;
    const next = prev + (proxy.t - prev) * k;
    playhead.velocity = (next - prev) / dt;
    playhead.t = next;

    const scene = resolveScene(next);
    playhead.scene = scene;
    playhead.sceneProgress = sceneProgress(next, scene);
    if (scene !== useRig.getState().scene) useRig.getState().setScene(scene);
  };

  let running = false;

  function start(): void {
    if (running) return;
    running = true;
    last = 0;
    // Symmetric with stop(): lenis.stop() suspends its internal raf loop, so a
    // stop -> start cycle (e.g. the motion-toggle round trip) must resume it or
    // scroll stays frozen. gsap.ticker.lagSmoothing is intentionally left at its
    // default (500, 33) -- not disabled -- because onTick already clamps its own
    // dt to [0.001, 0.1] above; a global un-restored lagSmoothing(0) mutation
    // would outlive this rig instance and buy nothing on top of that clamp.
    lenis.start();
    gsap.ticker.add(onTick);
    ScrollTrigger.refresh();
  }

  function stop(): void {
    if (!running) return;
    running = false;
    gsap.ticker.remove(onTick);
    lenis.stop();
  }

  function seek(t: number): void {
    const c = clamp01(t);
    const max = ScrollTrigger.maxScroll(window);
    proxy.t = c;
    playhead.t = c;
    playhead.velocity = 0;
    const scene = resolveScene(c);
    playhead.scene = scene;
    playhead.sceneProgress = sceneProgress(c, scene);
    useRig.getState().setScene(scene);
    lenis.scrollTo(max * c, { immediate: true });
  }

  function destroy(): void {
    stop();
    tween.scrollTrigger?.kill();
    tween.kill();
    lenis.destroy();
  }

  return { start, stop, destroy, seek };
}
