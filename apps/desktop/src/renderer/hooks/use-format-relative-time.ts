import type { TFunction } from '@ajh/translations';

/**
 * Logical relative-time tiers, in ascending order. The hook walks these and
 * returns the first whose threshold the elapsed time falls under.
 */
type RelativeKey = 'justNow' | 'minutesAgo' | 'hoursAgo' | 'daysAgo' | 'weeksAgo' | 'monthsAgo';

/**
 * Per-namespace key map. Two namespaces ship today and they use different
 * suffix conventions, so we resolve the full i18n key per logical tier here
 * rather than templating `${ns}.${tier}` (which only works for one shape).
 * Both maps keep the pre-existing keys intact so no translation breaks.
 */
// Job queue — flat `time*` keys under the `jobs` namespace (pre-existing).
// Kept as a standalone const so it can serve as the non-indexed default below
// (indexing `KEY_MAPS.jobs` would itself be `T | undefined` under
// `noUncheckedIndexedAccess`, defeating the fallback).
const JOBS_KEYS: Record<RelativeKey, string> = {
  justNow: 'jobs.timeJustNow',
  minutesAgo: 'jobs.timeMinutesAgo',
  hoursAgo: 'jobs.timeHoursAgo',
  daysAgo: 'jobs.timeDaysAgo',
  weeksAgo: 'jobs.timeWeeksAgo',
  monthsAgo: 'jobs.timeMonthsAgo',
};

const KEY_MAPS: Record<string, Record<RelativeKey, string>> = {
  jobs: JOBS_KEYS,
  // Résumé activity / generation cards — nested `relativeTime` namespace.
  'resumes.relativeTime': {
    justNow: 'resumes.relativeTime.justNow',
    minutesAgo: 'resumes.relativeTime.minutesAgo',
    hoursAgo: 'resumes.relativeTime.hoursAgo',
    daysAgo: 'resumes.relativeTime.daysAgo',
    weeksAgo: 'resumes.relativeTime.weeksAgo',
    monthsAgo: 'resumes.relativeTime.monthsAgo',
  },
};

const MINUTE = 60_000;

/**
 * Shared relative-time formatter. Returns a `(timestamp) => string` formatter
 * that buckets the elapsed time into minute / hour / day / week / month tiers
 * and localizes each via `t`. Pass the namespace whose i18n keys to use
 * (defaults to `jobs`); the `resumes.relativeTime` namespace is also supported.
 *
 * A falsy timestamp formats to an empty string (callers render nothing).
 */
export function useFormatRelativeTime(t: TFunction, nsPrefix: string = 'jobs') {
  const keys = KEY_MAPS[nsPrefix] ?? JOBS_KEYS;
  return (timestamp?: number): string => {
    if (!timestamp) return '';
    const diff = Date.now() - timestamp;
    const minutes = Math.floor(diff / MINUTE);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    const weeks = Math.floor(days / 7);
    const months = Math.floor(days / 30);

    if (minutes < 1) return t(keys.justNow);
    if (minutes < 60) return t(keys.minutesAgo, { m: minutes });
    if (hours < 24) return t(keys.hoursAgo, { h: hours });
    if (days < 7) return t(keys.daysAgo, { d: days });
    if (weeks < 4) return t(keys.weeksAgo, { w: weeks });
    return t(keys.monthsAgo, { m: months });
  };
}
