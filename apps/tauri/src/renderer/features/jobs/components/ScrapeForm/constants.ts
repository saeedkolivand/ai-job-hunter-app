import type { DATE_FILTER_OPTIONS } from '@ajh/shared';

export interface ScrapeFormState {
  board: string;
  query: string;
  location: string;
  /** Structured location captured from a picked geocode suggestion (#49/#40). */
  countryCode?: string;
  latitude?: number;
  longitude?: number;
  /** Search radius in km; 0 = exact location (no radius). */
  radiusKm: number;
  /** Target number of jobs to fetch (#41); sent as the scrape `amount` (backend clamps to 100). */
  amount: number;
  dateFilter: '' | (typeof DATE_FILTER_OPTIONS)[number];
  locale: string;
}

export const REGIONS = [
  { value: 'us', labelKey: 'jobs.regions.us' },
  { value: 'de', labelKey: 'jobs.regions.de' },
  { value: 'uk', labelKey: 'jobs.regions.uk' },
  { value: 'fr', labelKey: 'jobs.regions.fr' },
  { value: 'at', labelKey: 'jobs.regions.at' },
  { value: 'ch', labelKey: 'jobs.regions.ch' },
  { value: 'au', labelKey: 'jobs.regions.au' },
  { value: 'ca', labelKey: 'jobs.regions.ca' },
  { value: 'nl', labelKey: 'jobs.regions.nl' },
  { value: 'be', labelKey: 'jobs.regions.be' },
  { value: 'es', labelKey: 'jobs.regions.es' },
  { value: 'it', labelKey: 'jobs.regions.it' },
  { value: 'pl', labelKey: 'jobs.regions.pl' },
  { value: 'br', labelKey: 'jobs.regions.br' },
  { value: 'in', labelKey: 'jobs.regions.in' },
  { value: 'sg', labelKey: 'jobs.regions.sg' },
  { value: 'jp', labelKey: 'jobs.regions.jp' },
] as const;
