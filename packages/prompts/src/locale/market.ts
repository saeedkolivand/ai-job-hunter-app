import { hasLetterConventions } from './letter-conventions.js';

/**
 * ISO-3166 alpha-2 country → market id. Country splits that matter (US vs UK,
 * DE/AT/CH) are explicit; English-speaking peers map to the closest convention
 * set, and most Spanish-/Portuguese-speaking countries share es/pt.
 */
const COUNTRY_TO_MARKET: Record<string, string> = {
  US: 'us',
  GB: 'uk',
  UK: 'uk',
  IE: 'uk',
  AU: 'uk',
  NZ: 'uk',
  CA: 'us',
  IN: 'uk',
  SG: 'uk',
  DE: 'de',
  AT: 'at',
  CH: 'ch',
  FR: 'fr',
  BE: 'fr',
  LU: 'fr',
  MC: 'fr',
  ES: 'es',
  MX: 'es',
  AR: 'es',
  CL: 'es',
  CO: 'es',
  PE: 'es',
  IT: 'it',
  PT: 'pt',
  BR: 'br',
  TR: 'tr',
  RU: 'ru',
  CN: 'cn',
  TW: 'cn',
  HK: 'cn',
  JP: 'jp',
  KR: 'kr',
};

/** Letter language (ISO-639-1) → default market when no country is known. */
const LANGUAGE_TO_MARKET: Record<string, string> = {
  en: 'intl',
  de: 'de',
  fr: 'fr',
  es: 'es',
  it: 'it',
  pt: 'pt',
  tr: 'tr',
  ru: 'ru',
  zh: 'cn',
  ja: 'jp',
  ko: 'kr',
  nl: 'intl',
};

/** Map an ISO-3166 alpha-2 country code to a market id (undefined when unknown). */
export function countryToMarket(country?: string): string | undefined {
  if (!country) return undefined;
  return COUNTRY_TO_MARKET[country.trim().toUpperCase()];
}

/**
 * ISO-3166 alpha-2 country → ISO-4217 currency code. Wider than
 * {@link COUNTRY_TO_MARKET} (also covers Nordics/CEE/Eurozone countries with
 * no distinct letter-conventions entry) — grounds the web-researched salary
 * range (see `salary_research` on the Rust side) in the job's actual currency
 * so a blank/weak location can't let the model default to USD or hallucinate
 * one.
 *
 * ponytail: kept as its own map, not derived from {@link COUNTRY_TO_MARKET} —
 * the two group countries differently (e.g. all Eurozone members share one
 * currency but split across `de`/`fr`/`es`/`it` letter-convention markets), so
 * a derived map would need an inverse lookup with no real gain.
 */
const COUNTRY_TO_CURRENCY: Record<string, string> = {
  US: 'USD',
  GB: 'GBP',
  UK: 'GBP',
  IE: 'EUR',
  AU: 'AUD',
  NZ: 'NZD',
  CA: 'CAD',
  IN: 'INR',
  SG: 'SGD',
  DE: 'EUR',
  AT: 'EUR',
  CH: 'CHF',
  FR: 'EUR',
  BE: 'EUR',
  LU: 'EUR',
  MC: 'EUR',
  ES: 'EUR',
  MX: 'MXN',
  AR: 'ARS',
  CL: 'CLP',
  CO: 'COP',
  PE: 'PEN',
  IT: 'EUR',
  PT: 'EUR',
  BR: 'BRL',
  TR: 'TRY',
  RU: 'RUB',
  CN: 'CNY',
  TW: 'TWD',
  HK: 'HKD',
  JP: 'JPY',
  KR: 'KRW',
  SE: 'SEK',
  NO: 'NOK',
  DK: 'DKK',
  PL: 'PLN',
  CZ: 'CZK',
  SK: 'EUR',
  SI: 'EUR',
  EE: 'EUR',
  LV: 'EUR',
  LT: 'EUR',
  CY: 'EUR',
  MT: 'EUR',
  GR: 'EUR',
  FI: 'EUR',
  NL: 'EUR',
  HR: 'EUR',
  BG: 'EUR',
  HU: 'HUF',
  RO: 'RON',
  SM: 'EUR',
  VA: 'EUR',
  AD: 'EUR',
};

/** Map an ISO-3166 alpha-2 country code to its ISO-4217 currency (undefined when unknown). */
export function countryToCurrency(country?: string): string | undefined {
  if (!country) return undefined;
  return COUNTRY_TO_CURRENCY[country.trim().toUpperCase()];
}

export interface ResolveMarketInput {
  /** ISO-3166 alpha-2 country extracted from the job ad. */
  jobCountry?: string;
  /** Country inferred from the company-research brief HQ (fallback when the ad is silent). */
  briefCountry?: string;
  /** Letter target language (BCP-47 / ISO-639-1). */
  targetLanguage?: string;
  /** Explicit user-chosen market id (highest priority). */
  override?: string;
}

/**
 * Resolve the cover-letter market id. Priority: explicit override → job country
 * → research-brief HQ country → letter-language default → international. Always
 * returns a valid id that {@link letterConventions} can resolve.
 */
export function resolveMarket(input: ResolveMarketInput): string {
  const { jobCountry, briefCountry, targetLanguage, override } = input;
  if (override && hasLetterConventions(override)) return override.trim().toLowerCase();
  return (
    countryToMarket(jobCountry) ??
    countryToMarket(briefCountry) ??
    LANGUAGE_TO_MARKET[(targetLanguage ?? '').slice(0, 2).toLowerCase()] ??
    'intl'
  );
}

/** Segment delimiters for {@link countryFromLocation} — the country is conventionally
 *  the last comma/dash/slash/pipe/paren-separated chunk of a free-text location. */
const LOCATION_SEGMENT_SPLIT = /[,\-|/()]/;

/** Forms `Intl.DisplayNames` does not itself produce, keyed lowercase → the ISO code
 *  `COUNTRY_TO_MARKET` understands. Deliberately tiny — see the ponytail note below. */
const COUNTRY_NAME_ALIASES: Record<string, string> = {
  usa: 'US',
  'u.s.': 'US',
  'u.s.a.': 'US',
  'u.k.': 'GB',
  'great britain': 'GB',
  england: 'GB',
  scotland: 'GB',
  wales: 'GB',
};

// Reverse country-name → ISO-code index, memoised per locale at module scope
// (built lazily on first use of that locale, never rebuilt on every call).
const countryNameIndexCache = new Map<string, ReadonlyMap<string, string>>();

/** Reverse name→code index for one locale, over exactly the codes {@link COUNTRY_TO_MARKET}
 *  lists. An unsupported/malformed locale (or a runtime without full-ICU `DisplayNames`)
 *  degrades to an empty index rather than throwing. */
function countryNameIndex(locale: string): ReadonlyMap<string, string> {
  const key = locale.trim().toLowerCase();
  const cached = countryNameIndexCache.get(key);
  if (cached) return cached;
  const index = new Map<string, string>();
  try {
    const displayNames = new Intl.DisplayNames([locale], { type: 'region' });
    for (const code of Object.keys(COUNTRY_TO_MARKET)) {
      const name = displayNames.of(code);
      if (name) index.set(name.toLowerCase(), code);
    }
  } catch {
    // Empty index for this locale — countryFromLocation falls through to the
    // English index / alias table / no match.
  }
  countryNameIndexCache.set(key, index);
  return index;
}

/**
 * Extract an ISO-3166 alpha-2 country from a free-text job location (e.g.
 * "New York, NY, US", "Köln, Deutschland", "London, UK") for {@link resolveMarket}'s
 * `jobCountry` input — so an English posting in the US gets US Letter, not the
 * `en`→`intl` default's A4.
 *
 * ponytail: segment matching only — whole comma/dash/slash/pipe/paren-separated
 * segments, right-to-left (the country is conventionally last), never a substring
 * of the raw string (a substring match would let "Austin" hit "Austria"). No
 * fuzzy matching, no geocoding, and only the countries {@link COUNTRY_TO_MARKET}
 * already knows — not a general-purpose gazetteer.
 */
export function countryFromLocation(location?: string, language?: string): string | undefined {
  const trimmed = (location ?? '').trim();
  if (!trimmed) return undefined;

  const segments = trimmed
    .split(LOCATION_SEGMENT_SPLIT)
    .map((s) => s.trim())
    .filter(Boolean);

  const englishIndex = countryNameIndex('en');
  const langIndex = language ? countryNameIndex(language) : undefined;

  for (let i = segments.length - 1; i >= 0; i--) {
    const segment = segments[i];
    if (segment === undefined) continue;
    const lower = segment.toLowerCase();

    // Bare alpha-2 segment, e.g. "US" in "New York, NY, US" — must already be a
    // known key ("VA" in "Vienna, VA" is not one, so it's correctly rejected).
    if (segment.length === 2 && COUNTRY_TO_MARKET[segment.toUpperCase()]) {
      return segment.toUpperCase();
    }

    const alias = COUNTRY_NAME_ALIASES[lower];
    if (alias) return alias;

    const hit = englishIndex.get(lower) ?? langIndex?.get(lower);
    if (hit) return hit;
  }

  return undefined;
}
