import { MapPin } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

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
  const [rect, setRect] = useState<DOMRect | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const fetchRef = useRef(onFetchSuggestions);
  fetchRef.current = onFetchSuggestions;

  // Measure input position for the portal dropdown
  const measure = useCallback(() => {
    if (inputRef.current) setRect(inputRef.current.getBoundingClientRect());
  }, []);

  useEffect(() => {
    if (!open) return;
    measure();
    window.addEventListener('scroll', measure, true);
    window.addEventListener('resize', measure);
    return () => {
      window.removeEventListener('scroll', measure, true);
      window.removeEventListener('resize', measure);
    };
  }, [open, measure]);

  useEffect(() => {
    const trimmed = value.trim();
    if (trimmed.length < 2) {
      setSuggestions([]);
      setOpen(false);
      return;
    }
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      fetchRef
        .current(trimmed)
        .then((s) => {
          setSuggestions(s);
          setOpen(s.length > 0);
          setActiveIndex(-1);
          measure();
        })
        .catch(() => {});
    }, 300);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [value, measure]);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!inputRef.current?.contains(target) && !listRef.current?.contains(target)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

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
      const s = suggestions[activeIndex];
      if (s) {
        e.preventDefault();
        select(s);
      }
    } else if (e.key === 'Escape') {
      setOpen(false);
    }
  };

  const dropdownStyle: React.CSSProperties = rect
    ? { position: 'fixed', left: rect.left, top: rect.bottom + 4, width: rect.width, zIndex: 9999 }
    : { display: 'none' };

  return (
    <div className="relative">
      <div className="relative">
        <MapPin
          size={12}
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-foreground/30"
        />
        <input
          ref={inputRef}
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          onFocus={() => suggestions.length > 0 && setOpen(true)}
          placeholder={placeholder}
          disabled={disabled}
          autoComplete="off"
          className={cn(
            'input-field glass-dropdown w-full rounded-lg bg-white/[0.03] pl-8 pr-3 text-sm text-foreground placeholder:text-foreground/25',
            className
          )}
        />
      </div>

      {open &&
        suggestions.length > 0 &&
        createPortal(
          <ul
            ref={listRef}
            role="listbox"
            style={dropdownStyle}
            className="glass-modal overflow-hidden rounded-xl py-1.5 shadow-2xl shadow-black/60"
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
                  'flex cursor-pointer items-center gap-2.5 px-3 py-2 text-xs transition-colors duration-75',
                  i === activeIndex
                    ? 'bg-brand/20 text-foreground'
                    : 'text-foreground/60 hover:bg-white/[0.06] hover:text-foreground/90'
                )}
              >
                <MapPin size={11} className="shrink-0 text-foreground/35" />
                <span className="truncate">{s.display}</span>
              </li>
            ))}
          </ul>,
          document.body
        )}
    </div>
  );
}
