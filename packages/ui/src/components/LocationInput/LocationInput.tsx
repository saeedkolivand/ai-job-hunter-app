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
  onFetchSuggestions?: (query: string) => Promise<Suggestion[]>;
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
      setOpen(false);
    }
  };

  return (
    <div ref={triggerRef} className={className}>
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
