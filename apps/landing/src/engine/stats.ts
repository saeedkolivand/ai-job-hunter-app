// Dev-HUD telemetry bridge. The composer (which owns the renderer) writes the
// last frame's draw-call count here after composer.render(); the dev HUD reads
// it. A single shared mutable object, updated in place -- no per-frame
// allocation, and a no-op cost in production where the HUD never mounts.

export const hudStats = { calls: 0 };
