// Tunable rig constants. Starting values are skill-owned (see
// .claude/skills/webgl-standards/SKILL.md "The playhead model"); each may move
// within its ADR-0016 envelope during M1..M6. Pure data, no runtime deps.

// Total scroll runway expressed in svh (100 svh == one small-viewport height).
// 3000 svh == 30 viewport heights. ADR envelope: 2000..4000 svh.
export const SCROLL_TRACK_SVH = 3000;

// Film runtime the timecode counts up to: 2:40.
export const DURATION_SECONDS = 160;

// Playhead scrub smoothing (skill-owned, 0.5..1 band). Higher == snappier
// follow. Disabled entirely under reduced motion / the in-page motion toggle
// (that path is a chapter-stepped slideshow, not a damped scrub).
export const SCRUB = 0.6;

// The narrow breakpoint: CSS width must EXCEED this for the GL gate to pass.
export const NARROW_BREAKPOINT_PX = 900;
