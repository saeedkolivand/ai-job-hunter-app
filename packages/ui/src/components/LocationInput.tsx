import { MapPin } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { cn } from '../lib/cn';

interface Suggestion {
  display: string;
}

export interface LocationInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  /** Custom async fetcher — defaults to Nominatim via browser fetch (may be blocked by CSP in Tauri; pass a Tauri invoke wrapper instead). */
  onFetchSuggestions?: (query: string) => Promise<Suggestion[]>;
}

async function defaultFetch(query: string): Promise<Suggestion[]> {
  const url =
    `https://nominatim.openstreetmap.org/search` +
    `?q=${encodeURIComponent(query)}` +
    `&format=json&addressdetails=1&limit=6&featuretype=city`;
  try {
    const res = await fetch(url, { headers: { 'Accept-Language': 'en' } });
    if (!res.ok) return [];
    const data = (await res.json()) as Array<{
      address?: {
        city?: string;
        town?: string;
        village?: string;
        state?: string;
        country?: string;
      };
    }>;
    const seen = new Set<string>();
    return data.flatMap((item) => {
      const addr = item.address ?? {};
      const city = addr.city ?? addr.town ?? addr.village ?? '';
      if (!city) return [];
      const parts = [city, addr.state, addr.country].filter(Boolean);
      const display = parts.join(', ');
      if (seen.has(display)) return [];
      seen.add(display);
      return [{ display }];
    });
  } catch {
    return [];
  }
}

export function LocationInput({
  value,
  onChange,
  placeholder,
  disabled,
  className,
  onFetchSuggestions = defaultFetch,
}: LocationInputProps) {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const trimmed = value.trim();
    if (trimmed.length < 2) {
      setSuggestions([]);
      setOpen(false);
      return;
    }
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      onFetchSuggestions(trimmed)
        .then((s) => {
          setSuggestions(s);
          setOpen(s.length > 0);
          setActiveIndex(-1);
        })
        .catch(() => {});
    }, 300);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [value]);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  const select = (s: Suggestion) => {
    onChange(s.display);
    setOpen(false);
    setSuggestions([]);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (!open) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, suggestions.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === 'Enter' && activeIndex >= 0) {
      e.preventDefault();
      select(suggestions[activeIndex]);
    } else if (e.key === 'Escape') {
      setOpen(false);
    }
  };

  return (
    <div ref={containerRef} className="relative">
      <div className="relative">
        <MapPin
          size={11}
          className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-foreground/30"
        />
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          onFocus={() => suggestions.length > 0 && setOpen(true)}
          placeholder={placeholder}
          disabled={disabled}
          autoComplete="off"
          className={cn(
            'h-8 w-full rounded-md border border-white/[0.07] bg-white/[0.03] pl-7 pr-3 text-xs text-foreground outline-none transition-colors placeholder:text-foreground/25 focus:border-brand/40 focus:ring-1 focus:ring-brand/20 disabled:cursor-not-allowed disabled:opacity-50',
            className
          )}
        />
      </div>

      {open && suggestions.length > 0 && (
        <ul
          role="listbox"
          className="absolute z-50 mt-1 w-full overflow-hidden rounded-lg border border-white/[0.08] bg-[#1a1a2e] py-1 shadow-xl"
        >
          {suggestions.map((s, i) => (
            <li
              key={s.display}
              role="option"
              aria-selected={i === activeIndex}
              onMouseDown={(e) => {
                e.preventDefault();
                select(s);
              }}
              onMouseEnter={() => setActiveIndex(i)}
              className={cn(
                'flex cursor-pointer items-center gap-2 px-3 py-1.5 text-xs',
                i === activeIndex
                  ? 'bg-brand/15 text-foreground'
                  : 'text-foreground/70 hover:bg-white/[0.04]'
              )}
            >
              <MapPin size={10} className="shrink-0 text-foreground/30" />
              <span>{s.display}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
