import { useEffect, useRef, useState } from 'react';

interface Suggestion {
  display: string;
  lat?: number | null;
  lon?: number | null;
  countryCode?: string | null;
}

/**
 * Debounced location typeahead. The lookup itself is **always** the caller's —
 * this package deliberately has no built-in geocoder. It used to default to a
 * direct browser-side Nominatim call, which violated that endpoint's
 * no-autocomplete usage policy and shipped live in the published Storybook; the
 * desktop app passes the `geocode_suggest` Tauri command (bundled offline
 * GeoNames index, Photon fallback) instead.
 */
export function useGeocoding(
  query: string,
  onFetchSuggestions: (query: string) => Promise<Suggestion[]>
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
