import { ChevronDown, MapPin, Search, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { cn } from '../lib/cn';
import { transition } from '../lib/motion';
import { Button } from './Button';

interface Suggestion {
  display: string;
}

export interface LocationInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
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
  placeholder = 'Any location',
  disabled,
  className,
  onFetchSuggestions = defaultFetch,
}: LocationInputProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [position, setPosition] = useState({ top: 0, left: 0, width: 0 });

  const triggerRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const fetchRef = useRef(onFetchSuggestions);
  fetchRef.current = onFetchSuggestions;

  // Measure trigger position when opening
  useEffect(() => {
    if (open && triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      setPosition({ top: rect.bottom + 6, left: rect.left, width: rect.width });
      setQuery(value); // pre-fill with current value so user can edit
      setSuggestions([]);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open, value]);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (
        dropdownRef.current?.contains(e.target as Node) ||
        triggerRef.current?.contains(e.target as Node)
      )
        return;
      setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  // Debounced fetch
  useEffect(() => {
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
          setSuggestions(s);
          setActiveIndex(-1);
        })
        .catch(() => {});
    }, 300);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [query]);

  const select = (display: string) => {
    onChange(display);
    setOpen(false);
    setSuggestions([]);
  };

  const clear = (e: React.MouseEvent) => {
    e.stopPropagation();
    onChange('');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, suggestions.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === 'Enter') {
      const s = activeIndex >= 0 ? suggestions[activeIndex] : null;
      if (s) {
        e.preventDefault();
        select(s.display);
      } else if (query.trim()) {
        select(query.trim());
      }
    } else if (e.key === 'Escape') {
      setOpen(false);
    }
  };

  return (
    <div ref={triggerRef} className={className}>
      <Button
        type="button"
        disabled={disabled}
        onClick={() => !disabled && setOpen((o) => !o)}
        className={cn(
          'glass-graphite glass-highlight flex h-9 w-full items-center justify-between gap-2 rounded-xl px-3 text-xs transition-all duration-150',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50',
          open ? 'border-brand/35' : 'hover:bg-white/[0.02]'
        )}
      >
        <div className="flex min-w-0 items-center gap-2">
          <MapPin size={13} className="shrink-0 text-foreground/40" />
          <span className={cn('truncate', value ? 'text-foreground/90' : 'text-foreground/35')}>
            {value || placeholder}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {value && !disabled && (
            <span
              role="button"
              onClick={clear}
              className="rounded p-0.5 text-foreground/30 hover:text-foreground/70"
            >
              <X size={10} />
            </span>
          )}
          <ChevronDown
            size={12}
            className={cn(
              'text-foreground/30 transition-transform duration-150',
              open && 'rotate-180'
            )}
          />
        </div>
      </Button>

      {createPortal(
        <AnimatePresence>
          {open && (
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
              {/* Search input */}
              <div className="border-b border-white/[0.06] px-2 py-2">
                <div className="flex items-center gap-2 rounded-lg bg-white/[0.04] px-2.5 py-1.5">
                  <Search size={11} className="shrink-0 text-foreground/30" />
                  <input
                    ref={inputRef}
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder="Search city or postcode…"
                    className="flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-foreground/25"
                  />
                  {query && (
                    <button
                      type="button"
                      onClick={() => setQuery('')}
                      className="text-foreground/30 hover:text-foreground/60"
                    >
                      <X size={10} />
                    </button>
                  )}
                </div>
              </div>

              {/* Suggestions */}
              <div className="max-h-56 space-y-0.5 overflow-y-auto px-1 py-1">
                {suggestions.length === 0 && query.trim().length >= 2 ? (
                  <div className="px-3 py-4 text-center text-xs text-foreground/35">No results</div>
                ) : suggestions.length === 0 ? (
                  <div className="px-3 py-4 text-center text-xs text-foreground/25">
                    Type to search…
                  </div>
                ) : (
                  suggestions.map((s, i) => (
                    <button
                      key={s.display}
                      type="button"
                      onMouseDown={(e) => {
                        e.preventDefault();
                        select(s.display);
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
                  ))
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>,
        document.body
      )}
    </div>
  );
}
