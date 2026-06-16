/**
 * Accent color math
 * ─────────────────────────────────────────────────────────────────────────
 * Derives a coherent accent set from a single hex so a custom/system accent
 * never ships a broken-looking or unreadable UI:
 *   • brand-soft — the lighter step used for accent text/icons.
 *   • action-foreground — an auto-contrast label color (dark on pale accents,
 *     near-white on dark accents) so filled primary CTAs stay legible.
 * brand-dim + glows derive from --color-brand via CSS color-mix, so they need
 * no JS. Used by the theme engine's runtime accent applier.
 * ─────────────────────────────────────────────────────────────────────────
 */

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/** Parse `#rgb` / `#rrggbb` (leading # optional). Returns null when invalid. */
export function parseHex(hex: string): Rgb | null {
  let h = hex.trim().replace(/^#/, '');
  if (h.length === 3)
    h = h
      .split('')
      .map((c) => c + c)
      .join('');
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return null;
  const n = Number.parseInt(h, 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
}

const clamp255 = (v: number) => Math.round(Math.max(0, Math.min(255, v)));

export function toHex({ r, g, b }: Rgb): string {
  return '#' + [r, g, b].map((v) => clamp255(v).toString(16).padStart(2, '0')).join('');
}

/** WCAG sRGB relative luminance, 0 (black) … 1 (white). */
export function luminance({ r, g, b }: Rgb): number {
  const lin = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/** Mix a color toward white by `amount` (0..1). */
export function lighten(c: Rgb, amount: number): Rgb {
  const a = Math.max(0, Math.min(1, amount));
  return { r: c.r + (255 - c.r) * a, g: c.g + (255 - c.g) * a, b: c.b + (255 - c.b) * a };
}

/** Lighten a hex toward white; null on invalid input. */
export function lightenHex(hex: string, amount: number): string | null {
  const rgb = parseHex(hex);
  return rgb ? toHex(lighten(rgb, amount)) : null;
}

/**
 * Blend two hexes channel-wise by `t` (0 → all `a`, 1 → all `b`; default 0.5 =
 * even midpoint). Used to derive the sweep MIDDLE stop (--color-brand-mid) from
 * a custom/system accent's start↔end pair so the gradient reads as a smooth
 * teal→mid→rose with no shipped gold injected. Null when either hex is invalid.
 */
export function mixHex(a: string, b: string, t = 0.5): string | null {
  const ca = parseHex(a);
  const cb = parseHex(b);
  if (!ca || !cb) return null;
  const k = Math.max(0, Math.min(1, t));
  return toHex({
    r: ca.r + (cb.r - ca.r) * k,
    g: ca.g + (cb.g - ca.g) * k,
    b: ca.b + (cb.b - ca.b) * k,
  });
}

/**
 * A label color that stays legible on a filled accent: pale/bright accents get
 * a dark label, dark accents a near-white one. Returns CSS colors matching the
 * --color-action-foreground token family; null on invalid input.
 */
export function readableForeground(hex: string): string | null {
  const rgb = parseHex(hex);
  if (!rgb) return null;
  return luminance(rgb) > 0.55 ? '#1d1d1f' : '#ffffff';
}

/**
 * Accent gradient math
 * ─────────────────────────────────────────────────────────────────────────
 * Hue rotation lets a single accent hex spawn a coherent two-tone gradient
 * end: rotate the hue, keep saturation + lightness, and the pair reads as the
 * same family. Used by the theme engine to auto-derive --color-brand-2 when a
 * preset/custom accent ships no hand-tuned second hex.
 * ─────────────────────────────────────────────────────────────────────────
 */

/** RGB → HSL. h in degrees 0..360, s/l in 0..1. */
function rgbToHsl(c: Rgb): { h: number; s: number; l: number } {
  const r = c.r / 255;
  const g = c.g / 255;
  const b = c.b / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return { h: 0, s: 0, l };
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h: number;
  if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return { h: h * 60, s, l };
}

/** HSL → RGB. h in degrees, s/l in 0..1. */
function hslToRgb(h: number, s: number, l: number): Rgb {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const hp = (((h % 360) + 360) % 360) / 60;
  const x = c * (1 - Math.abs((hp % 2) - 1));
  // hp ∈ [0, 6) after normalization, so the sextant chain is exhaustive.
  const sextant = (): [number, number, number] => {
    if (hp < 1) return [c, x, 0];
    if (hp < 2) return [x, c, 0];
    if (hp < 3) return [0, c, x];
    if (hp < 4) return [0, x, c];
    if (hp < 5) return [x, 0, c];
    return [c, 0, x];
  };
  const [r1, g1, b1] = sextant();
  const m = l - c / 2;
  return { r: (r1 + m) * 255, g: (g1 + m) * 255, b: (b1 + m) * 255 };
}

/**
 * Rotate a hex's hue by `deg` (keeps S + L); null on invalid input. A non-finite
 * `deg` (NaN/±Infinity) is a no-op that returns the normalized original rather
 * than producing NaN channel garbage.
 */
export function rotateHueHex(hex: string, deg: number): string | null {
  const rgb = parseHex(hex);
  if (!rgb) return null;
  if (!Number.isFinite(deg)) return toHex(rgb);
  const { h, s, l } = rgbToHsl(rgb);
  return toHex(hslToRgb(h + deg, s, l));
}
