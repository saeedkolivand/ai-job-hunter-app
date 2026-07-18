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
// The teardown is idempotent and also runs if setup throws mid-init (so the
// pacing class / listeners / ticker hook can't leak into the legacy fallback),
// and it restores the SHARED GSAP ticker's default lagSmoothing that init turned
// off. Nothing touches window at module scope (SSR-safe).

import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import Lenis from "lenis";

import { channels } from "./channels";
import { activePageFor, EXIT_START } from "./pages";
import { setScroll } from "./store";

export function initScroll(): () => void {
  if (typeof window === "undefined") return () => {};

  gsap.registerPlugin(ScrollTrigger);

  const root = document.getElementById("semantic-root");

  let lenis: Lenis | undefined;
  let raf: ((time: number) => void) | undefined;
  let st: ScrollTrigger | undefined;
  let master: ReturnType<typeof gsap.timeline> | undefined;
  let onResize: (() => void) | undefined;
  let resizeTimer: ReturnType<typeof setTimeout> | undefined;

  // Idempotent teardown: undoes every side effect and restores the shared GSAP
  // ticker's DEFAULT lagSmoothing (500, 33) that init disabled globally. Runs
  // both as the returned cleanup and from the init catch below.
  let torn = false;
  const teardown = () => {
    if (torn) return;
    torn = true;
    if (onResize) window.removeEventListener("resize", onResize);
    if (resizeTimer !== undefined) clearTimeout(resizeTimer);
    lenis?.off("scroll", ScrollTrigger.update);
    if (raf) gsap.ticker.remove(raf);
    gsap.ticker.lagSmoothing(500, 33);
    st?.kill();
    master?.kill();
    lenis?.destroy();
    root?.classList.remove("gl-paced");
  };

  try {
    // Stretch the (hidden) semantic sections so scroll height paces the 9 pages.
    root?.classList.add("gl-paced");

    const l = new Lenis();
    lenis = l;

    // GSAP ticker drives Lenis; lagSmoothing off keeps scrub deterministic
    // (restored to the default in teardown -- the ticker is global).
    const r = (time: number) => l.raf(time * 1000);
    raf = r;
    gsap.ticker.add(r);
    gsap.ticker.lagSmoothing(0);

    l.on("scroll", ScrollTrigger.update);

    // The master timeline: page i occupies timeline time [i, i+1]. p ramps across
    // the whole unit; exitP ramps only across [i + EXIT_START, i + 1]. paused so
    // it never auto-plays -- ScrollTrigger drives its playhead from scroll.
    const tl = gsap.timeline({ paused: true });
    master = tl;
    let i = 0;
    for (const ch of channels) {
      tl.fromTo(ch, { p: 0 }, { p: 1, duration: 1, ease: "none" }, i);
      tl.fromTo(
        ch,
        { exitP: 0 },
        { exitP: 1, duration: 1 - EXIT_START, ease: "none" },
        i + EXIT_START,
      );
      i += 1;
    }

    const trigger = ScrollTrigger.create({
      trigger: document.body,
      start: 0,
      end: "max",
      scrub: true,
      animation: tl,
      onUpdate: (self) => {
        const t = self.progress;
        setScroll(t, l.velocity, activePageFor(t));
      },
    });
    st = trigger;

    // Height is only correct once webfonts have laid out; refresh again on resize.
    document.fonts.ready.then(() => ScrollTrigger.refresh());

    const handleResize = () => {
      if (resizeTimer !== undefined) clearTimeout(resizeTimer);
      resizeTimer = setTimeout(() => ScrollTrigger.refresh(), 150);
    };
    onResize = handleResize;
    window.addEventListener("resize", handleResize);

    // Debug ride: ?t=0.42 jumps straight to that progress once layout is settled.
    const param = new URLSearchParams(window.location.search).get("t");
    if (param !== null) {
      const target = Number(param);
      if (Number.isFinite(target)) {
        const clamped = Math.min(1, Math.max(0, target));
        ScrollTrigger.refresh();
        l.scrollTo(trigger.end * clamped, { immediate: true });
      }
    }
  } catch (err) {
    teardown();
    throw err;
  }

  return teardown;
}
