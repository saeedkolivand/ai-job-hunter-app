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
 * A label color that stays legible on a filled accent: pale/bright accents get
 * a dark label, dark accents a near-white one. Returns CSS colors matching the
 * --color-action-foreground token family; null on invalid input.
 */
export function readableForeground(hex: string): string | null {
  const rgb = parseHex(hex);
  if (!rgb) return null;
  return luminance(rgb) > 0.55 ? '#1d1d1f' : '#ffffff';
}
