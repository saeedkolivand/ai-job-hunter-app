import type { DATE_FILTER_OPTIONS } from '@ajh/shared';

export interface ScrapeFormState {
  board: string;
  query: string;
  location: string;
  pages: number;
  dateFilter: '' | (typeof DATE_FILTER_OPTIONS)[number];
  locale: string;
}

/** Boards whose results improve when the user is authenticated. */
export const AUTH_BENEFITS = new Set(['linkedin', 'indeed', 'xing']);

export const BOARDS = [
  { id: 'linkedin', labelKey: 'jobs.boards.linkedin' },
  { id: 'indeed', labelKey: 'jobs.boards.indeed' },
  { id: 'stepstone', labelKey: 'jobs.boards.stepstone' },
  { id: 'xing', labelKey: 'jobs.boards.xing' },
  { id: 'arbeitsagentur', labelKey: 'jobs.boards.arbeitsagentur' },
  { id: 'berlinstartupjobs', labelKey: 'jobs.boards.berlinstartupjobs' },
  { id: 'germantechjobs', labelKey: 'jobs.boards.germantechjobs' },
  { id: 'greenhouse', labelKey: 'jobs.boards.greenhouse' },
  { id: 'lever', labelKey: 'jobs.boards.lever' },
  { id: 'ashby', labelKey: 'jobs.boards.ashby' },
  { id: 'workday', labelKey: 'jobs.boards.workday' },
  { id: 'smartrecruiters', labelKey: 'jobs.boards.smartrecruiters' },
  { id: 'recruitee', labelKey: 'jobs.boards.recruitee' },
  { id: 'personio', labelKey: 'jobs.boards.personio' },
  { id: 'remoteok', labelKey: 'jobs.boards.remoteok' },
  { id: 'remotive', labelKey: 'jobs.boards.remotive' },
  { id: 'arbeitnow', labelKey: 'jobs.boards.arbeitnow' },
  { id: 'wwr', labelKey: 'jobs.boards.wwr' },
  { id: 'ycombinator', labelKey: 'jobs.boards.ycombinator' },
] as const;

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
