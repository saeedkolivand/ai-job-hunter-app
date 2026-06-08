import { CornerDownLeft, MapPin, Search, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { cn } from '../../lib/cn';
import { transition } from '../../lib/motion';

interface Suggestion {
  display: string;
  lat?: number | null;
  lon?: number | null;
  countryCode?: string | null;
}

export function LocationDropdown({
  open,
  position,
  query,
  setQuery,
  suggestions,
  activeIndex,
  setActiveIndex,
  onSelect,
  inputRef,
  dropdownRef,
  onKeyDown,
}: {
  open: boolean;
  position: { top: number; left: number; width: number };
  query: string;
  setQuery: (value: string) => void;
  suggestions: Suggestion[];
  activeIndex: number;
  setActiveIndex: (index: number) => void;
  onSelect: (suggestion: Suggestion) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  dropdownRef: React.RefObject<HTMLDivElement | null>;
  onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
}) {
  const trimmed = query.trim();
  const hasExactMatch = suggestions.some((s) => s.display.toLowerCase() === trimmed.toLowerCase());
  const showCustom = trimmed.length > 0 && !hasExactMatch;

  return (
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
            width: Math.min(Math.max(position.width, 240), 420),
            zIndex: 9999,
          }}
          className="dropdown-surface overflow-hidden rounded-xl"
        >
          {/* Search input */}
          <div className="border-b border-white/[0.06] px-2 py-2">
            <div className="flex items-center gap-2 rounded-lg bg-white/[0.04] px-2.5 py-1.5">
              <Search size={11} className="shrink-0 text-foreground/30" />
              <input
                ref={inputRef}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={onKeyDown}
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
            {showCustom && (
              <button
                type="button"
                onMouseDown={(e) => {
                  e.preventDefault();
                  onSelect({ display: trimmed });
                }}
                onMouseEnter={() => setActiveIndex(-1)}
                className={cn(
                  'flex w-full items-center gap-2 rounded-lg px-3 py-2 text-xs transition-colors',
                  activeIndex < 0
                    ? 'bg-brand/15 text-brand-soft'
                    : 'text-foreground/70 hover:bg-white/[0.05] hover:text-foreground/90'
                )}
              >
                <CornerDownLeft size={11} className="shrink-0 text-foreground/35" />
                <span className="truncate">
                  Use “<span className="text-foreground/90">{trimmed}</span>”
                </span>
              </button>
            )}
            {suggestions.map((s, i) => (
              <button
                key={s.display}
                type="button"
                onMouseDown={(e) => {
                  e.preventDefault();
                  onSelect(s);
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
            {!showCustom && suggestions.length === 0 && (
              <div className="px-3 py-4 text-center text-xs text-foreground/25">
                Type to search…
              </div>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
