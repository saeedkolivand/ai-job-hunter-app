// Shared magic-number constants for the /agent-system page's layout-dependent
// effects (belt scrub, fleet-map link geometry, reveal-on-scroll). Named here
// so the same value can't drift between links.ts and belt.ts.

// Below this width the fleet map / assembly line switch to their stacked
// mobile layout (`.belt-vert` instead of `.belt-sticky`, links hidden).
export const MOBILE_BREAKPOINT_QUERY = '(max-width:780px)';

// Debounce for the fleet-map link-geometry recompute on window resize.
export const RESIZE_DEBOUNCE_MS = 160;

// Duration of the hero agent-count cubic-ease count-up animation.
export const COUNT_UP_DURATION_MS = 900;

// How long a `.copy-cmd` chip shows its "copied" state after a click.
export const COPIED_TIMEOUT_MS = 1300;

// IntersectionObserver tuning for `.reveal` / `[data-count]` elements.
export const REVEAL_IO_THRESHOLD = 0.16;
export const REVEAL_IO_ROOT_MARGIN = '0px 0px -8% 0px';
