// Luminance-velocity clamp -- the WCAG 2.3.1 photosensitivity mechanism the skill
// (webgl-standards "A11y / UX gates" -> Photosensitivity) mandates for the dark
// scenes. The scene TARGET luminance is a pure f(t) (water-layout.sceneLuminance),
// but because scrolling drives the playhead, a FAST scrub could compress a slow
// blackout -> bright fade into a >3 Hz strobe. This rate limiter eases the APPLIED
// luminance toward the target, capping the change per unit of REAL time (delta
// seconds) so a flash can never complete faster than the WCAG-safe rate no matter
// how fast t is scrubbed.
//
// EXPLICIT EXCEPTION to the scrub-determinism rule: this is the ONE real-time
// (delta-driven), a11y-mandated smoothing in the render path -- deliberately NOT a
// pure f(t) (it is distinct from, and additional to, the scroll-rig's playhead
// damping). Determinism still holds AT REST: when the target is steady the applied
// value converges EXACTLY to it (the within-reach branch snaps, no asymptote), so
// a paused playhead at any t renders the identical frame -- the down-and-rewind
// parity contract survives. Only the transient DURING a fast scrub is rate-limited,
// which is exactly the strobe the clamp exists to prevent.

export class LuminanceClamp {
  private applied: number;
  private readonly maxSlew: number; // luminance units per second
  private primed: boolean;

  constructor(initial: number, maxSlewPerSecond: number) {
    this.applied = Number.isFinite(initial) ? initial : 0;
    this.maxSlew = maxSlewPerSecond > 0 ? maxSlewPerSecond : 0;
    this.primed = false;
  }

  get value(): number {
    return this.applied;
  }

  // Hard-set the applied value to a target with no slew limit, and mark the
  // clamp primed. Used explicitly for a discrete seek / deep-link jump, where
  // there is no visual continuity to protect: a hard cut is not a strobe, so
  // rate-limiting it would just be a slow unwanted fade. step() also calls this
  // internally for the very first call after construction (see below), so a
  // fresh mount gets the same hard-cut treatment automatically.
  reset(target: number): void {
    if (Number.isFinite(target)) this.applied = target;
    this.primed = true;
  }

  // Advance one real frame: move the applied luminance toward `target` by at most
  // maxSlew * dt. Within reach -> snap exactly to target (determinism at rest).
  // Non-finite target or dt <= 0 -> hold (no change).
  //
  // MOUNT PRIMING: the FIRST call after construction (or after an explicit
  // reset()) hard-cuts to the target instead of slewing, by delegating to
  // reset(). Without this, a fresh component mount landing far from the
  // constructor's throwaway `initial` value -- a hash deep-link straight into
  // the blackout, or the reduce-motion restore GL remount -- would visibly fade
  // from `initial` toward the real target over the first ~maxSlew window: an
  // unwanted, WCAG-INVERTING flash right at boot, the opposite of what this
  // class exists to prevent. A discrete mount has no prior visual continuity to
  // protect, exactly like an explicit reset() -- so the very first sample just
  // IS the correct target, immediately.
  step(target: number, dtSeconds: number): number {
    const tgt = Number.isFinite(target) ? target : this.applied;
    if (!this.primed) {
      this.reset(tgt);
      return this.applied;
    }
    const dt = Number.isFinite(dtSeconds) && dtSeconds > 0 ? dtSeconds : 0;
    const maxDelta = this.maxSlew * dt;
    const diff = tgt - this.applied;
    if (diff > maxDelta) this.applied += maxDelta;
    else if (diff < -maxDelta) this.applied -= maxDelta;
    else this.applied = tgt;
    return this.applied;
  }
}
