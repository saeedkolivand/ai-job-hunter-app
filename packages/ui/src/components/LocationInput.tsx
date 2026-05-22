import { MapPin } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { cn } from '../lib/cn';
import { transition } from '../lib/motion';

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
  const [position, setPosition] = useState({ top: 0, left: 0, width: 0 });
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const fetchRef = useRef(onFetchSuggestions);
  fetchRef.current = onFetchSuggestions;

  const measure = useCallback(() => {
    if (inputRef.current) {
      const rect = inputRef.current.getBoundingClientRect();
      setPosition({ top: rect.bottom + 6, left: rect.left, width: rect.width });
    }
  }, []);

  useEffect(() => {
    if (open) measure();
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

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (
        dropdownRef.current?.contains(e.target as Node) ||
        inputRef.current?.contains(e.target as Node)
      )
        return;
      setOpen(false);
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

      {createPortal(
        <AnimatePresence>
          {open && suggestions.length > 0 && (
            <motion.div
              ref={dropdownRef}
              initial={{ opacity: 0, y: -6, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -6, scale: 0.98 }}
              transition={transition.fast}
              style={{
                position: 'fixed',
                top: position.top,
                left: position.left,
                width: position.width,
                zIndex: 9999,
              }}
              className="glass-elevated overflow-hidden rounded-xl shadow-2xl"
            >
              <div className="max-h-56 overflow-y-auto px-1 py-1 space-y-0.5">
                {suggestions.map((s, i) => (
                  <button
                    key={s.display}
                    type="button"
                    onMouseDown={(e) => {
                      e.preventDefault();
                      select(s);
                    }}
                    onMouseEnter={() => setActiveIndex(i)}
                    className={cn(
                      'flex w-full items-center gap-2 rounded-lg px-3 py-2 text-xs transition-colors',
                      i === activeIndex
                        ? 'bg-brand/15 text-brand-soft'
                        : 'text-foreground/70 hover:bg-white/[0.05] hover:text-foreground/90'
                    )}
                  >
                    <MapPin size={11} className="shrink-0 text-foreground/35" />
                    <span className="truncate">{s.display}</span>
                  </button>
                ))}
              </div>
            </motion.div>
          )}
        </AnimatePresence>,
        document.body
      )}
    </div>
  );
}
