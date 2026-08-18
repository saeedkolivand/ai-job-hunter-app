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
  /**
   * Location autocomplete, filtered to city-level and country-level results
   * only (`to_city_country` in
   * `apps/desktop/src-tauri/src/commands/geocoding.rs`) — a street or a venue
   * is never a job-search location. `display` reads `"City, Country"` for a city
   * and the bare country name for a country-level match.
   */
  suggest(query: string): Promise<GeocodeSuggestion[]>;
}

export const GEOCODE_CHANNELS = {
  suggest: 'geocode:suggest',
} as const;
