import { useEffect, useRef, useState } from 'react';

interface Suggestion {
  display: string;
  lat?: number | null;
  lon?: number | null;
  countryCode?: string | null;
}

async function defaultFetch(query: string): Promise<Suggestion[]> {
  const url =
    `https://nominatim.openstreetmap.org/search` +
    `?q=${encodeURIComponent(query)}` +
    `&format=json&addressdetails=1&limit=6`;
  try {
    const res = await fetch(url, { headers: { 'Accept-Language': 'en' } });
    if (!res.ok) return [];
    const data = (await res.json()) as Array<{
      addresstype?: string;
      address?: {
        city?: string;
        town?: string;
        village?: string;
        municipality?: string;
        hamlet?: string;
        state?: string;
        country?: string;
      };
    }>;
    const seen = new Set<string>();
    return data.flatMap((item) => {
      const addr = item.address ?? {};
      const city = addr.city ?? addr.town ?? addr.village ?? addr.municipality ?? addr.hamlet ?? '';
      // City-level: "City, Country". Country-level: just the country.
      // Anything else (road/POI/region) is dropped.
      let display: string;
      if (city) {
        display = [city, addr.country].filter(Boolean).join(', ');
      } else if (item.addresstype === 'country' && addr.country) {
        display = addr.country;
      } else {
        return [];
      }
      if (seen.has(display)) return [];
      seen.add(display);
      return [{ display }];
    });
  } catch {
    return [];
  }
}

export function useGeocoding(
  query: string,
  onFetchSuggestions: (query: string) => Promise<Suggestion[]> = defaultFetch
) {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [activeIndex, setActiveIndex] = useState(-1);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const fetchRef = useRef(onFetchSuggestions);
  fetchRef.current = onFetchSuggestions;

  // Debounced fetch
  useEffect(() => {
    // Marks this run superseded. The debounce only bounds when a request
    // STARTS — once one is in flight nothing cancels it — so a slow older
    // request could resolve after a faster newer one and overwrite the list
    // with suggestions for a query the user has already typed past (and reset
    // `activeIndex`, dropping their keyboard selection mid-interaction).
    // React always runs the previous cleanup before the next effect run, so a
    // superseded request is flagged even when the new run takes the
    // `< 2 chars` early return. Also prevents a setState after unmount.
    let cancelled = false;
    const trimmed = query.trim();
    if (trimmed.length < 2) {
      setSuggestions([]);
      return;
    }
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      fetchRef
        .current(trimmed)
        .then((s) => {
          if (cancelled) return;
          setSuggestions(s);
          setActiveIndex(-1);
        })
        .catch(() => {});
    }, 300);
    return () => {
      cancelled = true;
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [query]);

  return { suggestions, activeIndex, setActiveIndex };
}
