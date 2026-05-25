export interface GeocodeContract {
  suggest(query: string): Promise<Array<{ display: string }>>;
}

export const GEOCODE_CHANNELS = {
  suggest: 'geocode:suggest',
} as const;
