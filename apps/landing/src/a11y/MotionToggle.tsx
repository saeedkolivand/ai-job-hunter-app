"use client";

// The in-page motion toggle (OS prefers-reduced-motion misses many vestibular
// users, so we ship our own). It is a VISIBLE control. Flipping it drives the
// gl-live <-> slideshow transition state machine in <Experience>.

export function MotionToggle({ reduced, onToggle }: { reduced: boolean; onToggle: () => void }) {
  return (
    <button
      type="button"
      className="motion-toggle"
      aria-pressed={reduced}
      onClick={onToggle}
    >
      {reduced ? "Enable motion" : "Reduce motion"}
    </button>
  );
}
