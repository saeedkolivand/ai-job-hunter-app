/**
 * Localized country name for an ISO 3166-1 alpha-2 code via the native
 * `Intl.DisplayNames` (zero-dep). Two call sites need it: the autopilot wizard's
 * derived-country line and the per-board "note" chips.
 *
 * Note tokens carry a LOWERCASE code (e.g. `"de"`) while `DisplayNames` wants an
 * uppercase region code, so we normalize. An unknown/unsupported code, a
 * malformed input, or a runtime without full-ICU `DisplayNames` all degrade to
 * the uppercased code rather than throwing — the caller always gets a printable
 * string.
 */
export function regionName(code: string, locale: string): string {
  const cc = code.trim().toUpperCase();
  // Alpha-2 only (our tokens/geocode codes) — guards the empty/short inputs that
  // would otherwise throw a RangeError inside DisplayNames.
  if (cc.length !== 2) return cc;
  try {
    return new Intl.DisplayNames([locale], { type: 'region' }).of(cc) ?? cc;
  } catch {
    return cc;
  }
}
