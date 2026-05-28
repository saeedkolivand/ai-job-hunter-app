export function useFormatRelativeTime(
  t: (key: string, params?: Record<string, unknown>) => string
) {
  return (timestamp?: number): string => {
    if (!timestamp) return '';
    const now = Date.now();
    const diff = now - timestamp;
    const seconds = Math.floor(diff / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    const weeks = Math.floor(days / 7);
    const months = Math.floor(days / 30);

    if (minutes < 1) return t('jobs.timeJustNow');
    if (minutes < 60) return t('jobs.timeMinutesAgo', { m: minutes });
    if (hours < 24) return t('jobs.timeHoursAgo', { h: hours });
    if (days < 7) return t('jobs.timeDaysAgo', { d: days });
    if (weeks < 4) return t('jobs.timeWeeksAgo', { w: weeks });
    return t('jobs.timeMonthsAgo', { m: months });
  };
}
