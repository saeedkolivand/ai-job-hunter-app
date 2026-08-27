/**
 * Format a scraped salary range for the compact Applications row chip, e.g.
 * `€60,000–€80,000`. Only one bound present → that single figure; both equal →
 * one figure. Returns `null` when the posting carried no salary at all so call
 * sites render nothing.
 *
 * `locale` is optional and defaults to the host locale (`Intl`'s own default),
 * which matches how every other number in the renderer is formatted.
 */
export function formatSalaryRange(
  min?: number,
  max?: number,
  currency?: string,
  locale?: string
): string | null {
  const format = (value: number): string => {
    const trimmed = currency?.trim();
    if (trimmed) {
      try {
        return new Intl.NumberFormat(locale, {
          style: 'currency',
          currency: trimmed,
          maximumFractionDigits: 0,
        }).format(value);
      } catch {
        // A malformed / non-ISO-4217 code makes Intl throw (boards do send junk).
        // Fall through to the plain-number format rather than blanking the row.
      }
    }
    return new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(value);
  };

  if (min != null && max != null) {
    return min === max ? format(min) : `${format(min)}–${format(max)}`;
  }
  if (min != null) return format(min);
  if (max != null) return format(max);
  return null;
}
