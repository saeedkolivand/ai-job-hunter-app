export interface GeocodeSuggestion {
  display: string;
  /** WGS84 latitude of the place (for radius search). */
  lat?: number | null;
  /** WGS84 longitude of the place (for radius search). */
  lon?: number | null;
  /** ISO 3166-1 alpha-2 country code (upper-case) — for country-correct filtering (#49). */
  countryCode?: string | null;
}

export interface GeocodeContract {
  suggest(query: string): Promise<GeocodeSuggestion[]>;
}

export const GEOCODE_CHANNELS = {
  suggest: 'geocode:suggest',
} as const;
