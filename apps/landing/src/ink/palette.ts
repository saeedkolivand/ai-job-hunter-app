// The page ink palette -- the single source of truth for every GL ink scene.
// Hex strings mirror the semantic layer's CSS custom properties so the GL and
// DOM layers render the same colours. Kept as plain strings (not THREE.Color)
// so this module stays framework-free; scenes wrap them in a Color at the point
// of use (THREE.Color parses the sRGB hex and decodes to the linear working
// space automatically -- see the sRGB note in ink/boil.ts callers).

export const PALETTE = {
  paper: "#f4ecdc", // page stock behind everything
  ink: "#1c1812", // primary stroke / body ink
  red: "#e24b4a", // accent / alert marks
  blue: "#6cc6ff", // cool accent
  line: "#e7ecf3", // faint ruled-guide lines
} as const;

export type PaletteKey = keyof typeof PALETTE;
