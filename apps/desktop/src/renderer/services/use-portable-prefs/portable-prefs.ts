import { z } from 'zod';

import { DEFAULT_THEME_PREFS, type ThemePrefs } from '@ajh/ui';

import { type Preferences, PreferencesSchema } from '@/store/preferences-schema';
import { defaultPreferences } from '@/store/preferences-store';

/**
 * The renderer's side of the portable-preferences allowlist (#1433). The shell
 * keeps the authoritative list and rejects anything else; this file only maps
 * the zustand store (and the theme) to and from it. Everything not named here
 * (performance, local model limits, close-to-tray, consent toggles, secrets)
 * stays on the device.
 */
const PLAIN_KEYS = [
  'userName',
  'language',
  'outputTone',
  'resume',
  'applicant',
  'promptQuality',
  'recentLocations',
] as const satisfies readonly (keyof Preferences)[];

export type PortableValues = Record<string, unknown>;

// Key order must not matter: a stored theme and the default list the same keys differently.
const canon = (v: unknown) =>
  JSON.stringify(v, (_k, x: unknown) =>
    x && typeof x === 'object' && !Array.isArray(x)
      ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => a.localeCompare(b)))
      : x
  );
export const same = (a: unknown, b: unknown) => canon(a) === canon(b);

const HEX = /^#[0-9a-f]{6}$/i;
const ThemeSchema = z.object({
  scheme: z.enum(['light', 'dark', 'system']),
  reduceTransparency: z.boolean(),
  contrast: z.enum(['normal', 'more']),
  textScale: z.enum(['small', 'default', 'large']),
  accentSource: z.enum(['default', 'system', 'custom']),
  // The accent lands in a CSS variable: only a plain hex may come from another device.
  accentColor: z.string().regex(HEX).optional(),
  accentColor2: z.string().regex(HEX).optional(),
});
const EffortSchema = z.record(z.string(), z.string());

const effortOf = (cfg: Preferences['aiProviderConfig']) => {
  const out: Record<string, string> = {};
  for (const [provider, s] of Object.entries(cfg?.providers ?? {})) {
    if (s.effort) out[provider] = s.effort;
  }
  return out;
};

/** The portable slice of the store (the theme travels separately). */
export const portableOf = (p: Preferences): PortableValues => {
  const out: PortableValues = {};
  for (const k of PLAIN_KEYS) if (p[k] !== undefined) out[k] = p[k];
  // Always present (empty when none is set) so clearing the last effort is a change.
  out.providerEffort = effortOf(p.aiProviderConfig);
  return out;
};

/** Keys whose value differs between two states. */
export const changedKeys = (next: PortableValues, prev: PortableValues): PortableValues =>
  Object.fromEntries(Object.entries(next).filter(([k, v]) => !same(v, prev[k])));

/** Values the user changed from the defaults: the one-time copy to the backend. */
export const localForMigration = (p: Preferences, theme: ThemePrefs): PortableValues => {
  const defaults = portableOf(defaultPreferences);
  const out = changedKeys(portableOf(p), defaults);
  if (!same(theme, DEFAULT_THEME_PREFS)) out.theme = theme;
  return out;
};

/** What to change locally so it matches `values` (received or stored). */
export const hydrationOf = (
  values: PortableValues,
  current: Preferences,
  theme: ThemePrefs
): { patch: Partial<Preferences> | null; theme: ThemePrefs | null } => {
  const patch: Record<string, unknown> = {};
  for (const k of PLAIN_KEYS) {
    if (!(k in values)) continue;
    const parsed = (PreferencesSchema.shape[k] as z.ZodType).safeParse(values[k]);
    if (parsed.success && !same(parsed.data, current[k])) patch[k] = parsed.data;
  }
  const effort = EffortSchema.safeParse(values.providerEffort);
  if (effort.success) {
    const providers = { ...current.aiProviderConfig?.providers };
    let changed = false;
    // An effort the sender no longer has is cleared here too (model and limits stay).
    for (const [name, s] of Object.entries(providers)) {
      if (s.effort && !(name in effort.data)) {
        providers[name] = { ...s, effort: '' };
        changed = true;
      }
    }
    for (const [name, level] of Object.entries(effort.data)) {
      if (providers[name]?.effort === level) continue;
      providers[name] = { ...(providers[name] ?? { model: '' }), effort: level };
      changed = true;
    }
    if (changed) {
      patch.aiProviderConfig = {
        activeProvider: current.aiProviderConfig?.activeProvider ?? 'ollama',
        providers,
      };
    }
  }
  const parsedTheme = ThemeSchema.safeParse(values.theme);
  const nextTheme = parsedTheme.success ? { ...DEFAULT_THEME_PREFS, ...parsedTheme.data } : null;
  return {
    patch: Object.keys(patch).length ? patch : null,
    theme: nextTheme && !same(nextTheme, theme) ? nextTheme : null,
  };
};
