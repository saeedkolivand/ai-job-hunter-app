import { ChevronDown, MapPin, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { useGeocoding } from '../../hooks/useGeocoding';
import { cn } from '../../lib/cn';
import { Button } from '../Button';
import { LocationDropdown } from '../LocationDropdown';

interface Suggestion {
  display: string;
  lat?: number | null;
  lon?: number | null;
  countryCode?: string | null;
}

export interface LocationInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  /** Forwarded to the trigger button so an external `<label htmlFor>` resolves. */
  id?: string;
  /**
   * Required: this package ships no built-in geocoder. Callers pass their own
   * lookup (the desktop app passes the `geocode_suggest` Tauri command), so a
   * consumer can never accidentally emit browser-side traffic to a third-party
   * geocoding service just by omitting a prop.
   */
  onFetchSuggestions: (query: string) => Promise<Suggestion[]>;
  /**
   * Accessible name for the clear (×) button. Defaults to `'Clear'` — pass a
   * localized string from the consuming app.
   */
  clearLabel?: string;
  /**
   * Fires when a value is committed (suggestion picked, custom text, or cleared)
   * with the full structured suggestion — lets callers capture country/coords
   * for precise downstream filtering (#49/#40). A cleared/typed value carries
   * only `display`.
   */
  onSelectSuggestion?: (suggestion: Suggestion) => void;
}

export function LocationInput({
  value,
  onChange,
  placeholder = 'Any location',
  disabled,
  className,
  id,
  clearLabel = 'Clear',
  onFetchSuggestions,
  onSelectSuggestion,
}: LocationInputProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [position, setPosition] = useState({ top: 0, left: 0, width: 0 });

  const triggerRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const { suggestions, activeIndex, setActiveIndex } = useGeocoding(query, onFetchSuggestions);

  // Measure trigger position when opening
  useEffect(() => {
    if (open && triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      setPosition({ top: rect.bottom + 6, left: rect.left, width: rect.width });
      setQuery(value); // pre-fill with current value so user can edit
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

  const select = (suggestion: Suggestion) => {
    onChange(suggestion.display);
    onSelectSuggestion?.(suggestion);
    setOpen(false);
  };

  const clear = (e: React.MouseEvent) => {
    e.stopPropagation();
    onChange('');
    onSelectSuggestion?.({ display: '' });
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
        select(s);
      } else if (query.trim()) {
        select({ display: query.trim() });
      }
    } else if (e.key === 'Escape') {
      // Innermost-layer-wins (see `useDropdownKeyboard`): only an OPEN suggestion
      // panel consumes Escape, so it can't also reach an ancestor dialog/drawer
      // listening on `window` and close the whole surface. This input now renders
      // inside the scrape drawer via ScrapeFilters, where dismissing suggestions
      // used to tear down the drawer with them.
      if (open) e.stopPropagation();
      setOpen(false);
    }
  };

  const showClear = Boolean(value) && !disabled;

  return (
    <div ref={triggerRef} className={cn('relative', className)}>
      <Button
        id={id}
        type="button"
        variant="unstyled"
        disabled={disabled}
        onClick={() => !disabled && setOpen((o) => !o)}
        className={cn(
          // `unstyled` so the field doesn't inherit the Button base `active:scale`
          // press shrink — it looks like a text input, not a pressable button.
          'bg-field border border-[var(--border-clear)] flex h-9 w-full items-center justify-between gap-2 rounded-lg px-3 text-xs transition-colors duration-150',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50',
          open ? 'border-brand/35' : 'hover:bg-muted'
        )}
      >
        <div className="flex min-w-0 items-center gap-2">
          <MapPin size={13} className="shrink-0 text-foreground/40" />
          <span className={cn('truncate', value ? 'text-foreground/90' : 'text-foreground/35')}>
            {value || placeholder}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {showClear && (
            // Placeholder for the clear button, which is a SIBLING of this
            // trigger: a <button> nested in a <button> is invalid markup and
            // unreachable by keyboard. Reserving its 14px footprint here keeps
            // the truncated value from running underneath the hoisted control.
            <span aria-hidden="true" className="w-[14px]" />
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

      {showClear && (
        <Button
          type="button"
          variant="unstyled"
          aria-label={clearLabel}
          onClick={clear}
          // Overlays the placeholder above. The glyph used to sit 30–40px from
          // the field's right edge (12px padding + 12px chevron + 4px gap +
          // 2px inset); `p-2` + `right-[22px]` keeps its centre exactly there
          // (35px in) while taking the hit box to 10px + 2×8px = 26px, past the
          // 24px minimum (WCAG 2.5.8) that `p-1.5` missed at ~22px. Same trick
          // as the Tag close button.
          className="absolute right-[22px] top-1/2 flex -translate-y-1/2 items-center rounded p-2 text-foreground/30 hover:text-foreground/70"
        >
          <X size={10} />
        </Button>
      )}

      <LocationDropdown
        open={open}
        position={position}
        query={query}
        setQuery={setQuery}
        suggestions={suggestions}
        activeIndex={activeIndex}
        setActiveIndex={setActiveIndex}
        onSelect={select}
        inputRef={inputRef}
        dropdownRef={dropdownRef}
        onKeyDown={handleKeyDown}
      />
    </div>
  );
}
