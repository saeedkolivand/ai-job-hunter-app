// Window-level normalized pointer -- the source for the camera micro-parallax.
// The Canvas is pointer-events:none (so document scroll stays with Lenis and the
// future a11y overlay can sit above the canvas without the canvas eating its
// clicks), which means R3F's own state.pointer never updates. Parallax reads THIS
// instead, fed by a single window pointermove listener. x/y are NDC-style
// [-1,1] with y up (matching R3F's pointer convention). One shared mutable
// object, read once per frame in the composer loop -> no per-frame allocation.

export const pointer = { x: 0, y: 0 };

// Attach the window pointermove listener; returns its own teardown. SSR-safe (no
// window at module scope; guarded here) and passive (it never preventDefaults, so
// it can't interfere with scroll or selection).
export function trackPointer(): () => void {
  if (typeof window === "undefined") return () => {};
  const onMove = (e: PointerEvent) => {
    pointer.x = (e.clientX / window.innerWidth) * 2 - 1;
    pointer.y = -(e.clientY / window.innerHeight) * 2 + 1;
  };
  window.addEventListener("pointermove", onMove, { passive: true });
  return () => window.removeEventListener("pointermove", onMove);
}
