export type LocaleCode = 'en' | 'de';

export const LOCALE_CODES = {
  ENGLISH: 'en' as LocaleCode,
  GERMAN: 'de' as LocaleCode,
} as const;

export const LOCALES = [
  { code: LOCALE_CODES.ENGLISH, label: 'English', flag: '🇬🇧' },
  { code: LOCALE_CODES.GERMAN, label: 'Deutsch', flag: '🇩🇪' },
] as const;
