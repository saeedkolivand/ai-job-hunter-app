import type { TFunction } from '@ajh/translations';

export function getTimeGreeting(t: TFunction): string {
  const hour = new Date().getHours();
  if (hour < 12) return t('nav.greeting.morning');
  if (hour < 18) return t('nav.greeting.afternoon');
  return t('nav.greeting.evening');
}
