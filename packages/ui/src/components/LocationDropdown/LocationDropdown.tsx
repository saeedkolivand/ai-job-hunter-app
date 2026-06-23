import { CornerDownLeft, MapPin } from 'lucide-react';

import { cn } from '../../lib/cn';
import { DropdownPanel } from '../DropdownPanel';
import { DropdownSearch } from '../DropdownSearch';

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
    <DropdownPanel
      open={open}
      panelRef={dropdownRef}
      role="listbox"
      style={{
        position: 'fixed',
        top: position.top,
        left: position.left,
        width: Math.min(Math.max(position.width, 240), 420),
        zIndex: 9999,
      }}
    >
      <DropdownSearch
        search={query}
        setSearch={setQuery}
        searchRef={inputRef}
        placeholder="Search city or postcode…"
        onClear={() => setQuery('')}
        onKeyDown={onKeyDown}
      />

      <div className="max-h-56 space-y-0.5 overflow-y-auto px-1 py-1 scrollbar-thin">
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
                : 'text-foreground/70 hover:bg-muted hover:text-foreground/90'
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
                : 'text-foreground/70 hover:bg-muted hover:text-foreground/90'
            )}
          >
            <MapPin size={11} className="shrink-0 text-foreground/35" />
            <span className="truncate">{s.display}</span>
          </button>
        ))}
        {!showCustom && suggestions.length === 0 && (
          <div className="px-3 py-4 text-center text-xs text-foreground/25">Type to search…</div>
        )}
      </div>
    </DropdownPanel>
  );
}
