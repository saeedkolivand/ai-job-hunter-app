import type { DATE_FILTER_OPTIONS } from '@ajh/shared';

export interface ScrapeFormState {
  boards: string[];
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
  /**
   * Company slugs for ATS boards (greenhouse, lever, ashby, etc.) whose APIs
   * require a company identifier. Comma-separated in the UI, stored as an array.
   * Empty array = no filter; backend skips ATS boards with `needs-company`.
   */
  companies: string[];
}
