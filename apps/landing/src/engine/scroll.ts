// The scroll spine. Lenis owns document scroll; the prerendered semantic layer
// supplies the height (its sections are visibility:hidden, never display:none,
// so layout height survives -- and .gl-paced stretches each to a generous,
// evenly paced min-height for the 9-page t-space). GSAP's ticker drives Lenis
// (lagSmoothing off so scrub stays frame-locked), and every Lenis scroll forces
// ScrollTrigger.update so the two clocks never drift.
//
// A SINGLE ScrollTrigger (scrub:true) scrubs ONE master timeline of duration 9.
// The timeline is the only GSAP -> GL bridge: per-page it tweens channels[i].p
// across that page's whole slice and channels[i].exitP across its exit
// sub-slice, with ABSOLUTE fromTo tweens (ease none) and never += / -=, so
// scrubbing back down fully reverses every page. The ScrollTrigger onUpdate
// writes the global t / velocity / active-page index into the zustand store.
//
// initScroll() is call-once from a client effect and returns its own teardown.
// Nothing touches window at module scope (SSR-safe); the whole graph is built
// inside initScroll, which only ever runs client-side.

import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import Lenis from "lenis";

import { channels } from "./channels";
import { activePageFor, EXIT_START } from "./pages";
import { setScroll } from "./store";

export function initScroll(): () => void {
  if (typeof window === "undefined") return () => {};

  gsap.registerPlugin(ScrollTrigger);

  // Stretch the (hidden) semantic sections so scroll height paces the 9 pages.
  const root = document.getElementById("semantic-root");
  root?.classList.add("gl-paced");

  const lenis = new Lenis();

  // GSAP ticker drives Lenis; lagSmoothing off keeps scrub deterministic.
  const raf = (time: number) => lenis.raf(time * 1000);
  gsap.ticker.add(raf);
  gsap.ticker.lagSmoothing(0);

  lenis.on("scroll", ScrollTrigger.update);

  // The master timeline: page i occupies timeline time [i, i+1]. p ramps across
  // the whole unit; exitP ramps only across [i + EXIT_START, i + 1]. paused so
  // it never auto-plays -- ScrollTrigger drives its playhead from scroll.
  const master = gsap.timeline({ paused: true });
  let i = 0;
  for (const ch of channels) {
    master.fromTo(ch, { p: 0 }, { p: 1, duration: 1, ease: "none" }, i);
    master.fromTo(
      ch,
      { exitP: 0 },
      { exitP: 1, duration: 1 - EXIT_START, ease: "none" },
      i + EXIT_START,
    );
    i += 1;
  }

  const st = ScrollTrigger.create({
    trigger: document.body,
    start: 0,
    end: "max",
    scrub: true,
    animation: master,
    onUpdate: (self) => {
      const t = self.progress;
      setScroll(t, lenis.velocity, activePageFor(t));
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
    master.kill();
    lenis.destroy();
    root?.classList.remove("gl-paced");
  };
}
