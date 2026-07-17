// Scroll driver: binds Lenis smooth-scroll to GSAP ScrollTrigger and pumps the
// single global t into journeyStore. The hidden semantic layer supplies the
// document scroll height, so Lenis runs on the document (no wrapper) and the
// one ScrollTrigger spans document.body 0..max. GSAP's ticker owns the RAF loop
// (lagSmoothing off so scrub stays frame-locked), and every Lenis scroll event
// forces ScrollTrigger.update so the two clocks never drift.
//
// initScroll() is call-once from a client effect and returns its own cleanup.
// Nothing here touches window at module scope (SSR-safe); the whole graph is
// built inside initScroll, which only ever runs client-side.

import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import Lenis from "lenis";

import { journeyStore } from "./store";

export function initScroll(): () => void {
  if (typeof window === "undefined") return () => {};

  gsap.registerPlugin(ScrollTrigger);

  const lenis = new Lenis();

  // GSAP ticker drives Lenis; lagSmoothing off keeps scrub deterministic.
  const raf = (time: number) => lenis.raf(time * 1000);
  gsap.ticker.add(raf);
  gsap.ticker.lagSmoothing(0);

  lenis.on("scroll", ScrollTrigger.update);

  const st = ScrollTrigger.create({
    trigger: document.body,
    start: 0,
    end: "max",
    scrub: true,
    onUpdate: (self) => {
      journeyStore.setState({ t: self.progress, vel: lenis.velocity });
    },
  });

  // Height is only correct once webfonts have laid out; refresh again on resize.
  document.fonts.ready.then(() => ScrollTrigger.refresh());

  let resizeTimer: ReturnType<typeof setTimeout> | undefined;
  const onResize = () => {
    if (resizeTimer !== undefined) clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => ScrollTrigger.refresh(), 150);
  };
  window.addEventListener("resize", onResize);

  // Debug ride: ?t=0.42 jumps straight to that progress once layout is settled.
  const param = new URLSearchParams(window.location.search).get("t");
  if (param !== null) {
    const target = Number(param);
    if (Number.isFinite(target)) {
      const clamped = Math.min(1, Math.max(0, target));
      ScrollTrigger.refresh();
      lenis.scrollTo(st.end * clamped, { immediate: true });
    }
  }

  return () => {
    window.removeEventListener("resize", onResize);
    if (resizeTimer !== undefined) clearTimeout(resizeTimer);
    lenis.off("scroll", ScrollTrigger.update);
    gsap.ticker.remove(raf);
    st.kill();
    lenis.destroy();
  };
}
