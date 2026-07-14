/**
 * Relative-time formatting (#45) — "1 min ago", "1 hr ago", "3 days ago".
 *
 * Locale-aware via the built-in `Intl.RelativeTimeFormat` (no dependency). Kept
 * a pure utility (no i18n side-effect import): callers pass the active i18n
 * language as `locale`; otherwise it falls back to the document/runtime locale.
 */

type Unit = Intl.RelativeTimeFormatUnit;

const DIVISIONS: { amount: number; unit: Unit }[] = [
  { amount: 60, unit: 'second' },
  { amount: 60, unit: 'minute' },
  { amount: 24, unit: 'hour' },
  { amount: 7, unit: 'day' },
  { amount: 4.34524, unit: 'week' },
  { amount: 12, unit: 'month' },
  { amount: Number.POSITIVE_INFINITY, unit: 'year' },
];

function resolveLocale(locale?: string): string {
  if (locale) return locale;
  if (typeof document !== 'undefined' && document.documentElement.lang)
    return document.documentElement.lang;
  if (typeof navigator !== 'undefined' && navigator.language) return navigator.language;
  return 'en';
}

/**
 * Format a past (or future) timestamp relative to `now`. Accepts a `Date`, an
 * epoch-ms number, or a date string. Returns `''` for an unparseable input.
 */
export function timeAgo(
  input: Date | number | string,
  now: number = Date.now(),
  locale?: string
): string {
  const ts =
    input instanceof Date ? input.getTime() : typeof input === 'number' ? input : Date.parse(input);
  if (Number.isNaN(ts)) return '';

  const rtf = new Intl.RelativeTimeFormat(resolveLocale(locale), {
    numeric: 'auto',
    style: 'short',
  });

  let duration = (ts - now) / 1000; // seconds; negative = in the past
  for (const { amount, unit } of DIVISIONS) {
    if (Math.abs(duration) < amount) return rtf.format(Math.round(duration), unit);
    duration /= amount;
  }
  return '';
}

/**
 * Parses a date that may be a date-only `YYYY-MM-DD` string (as the bundled
 * changelog's `publishedAt` uses — see `updater/mod.rs`) or a full ISO
 * timestamp. `new Date('YYYY-MM-DD')` parses as UTC midnight, which renders as
 * the previous day for anyone west of UTC — treat a date-only string as a
 * local calendar date instead. Full timestamps are passed through unchanged.
 */
export function parseCalendarOrIsoDate(value: string): Date {
  return /^\d{4}-\d{2}-\d{2}$/.test(value) ? new Date(`${value}T00:00:00`) : new Date(value);
}
