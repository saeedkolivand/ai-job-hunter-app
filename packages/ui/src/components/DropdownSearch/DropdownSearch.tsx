import { Search, X } from 'lucide-react';

interface DropdownSearchProps {
  search: string;
  setSearch: (value: string) => void;
  searchRef: React.RefObject<HTMLInputElement | null>;
  placeholder?: string;
  /** When provided, renders a trailing clear (X) button while `search` is non-empty. */
  onClear?: () => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
}

export function DropdownSearch({
  search,
  setSearch,
  searchRef,
  placeholder = 'Search…',
  onClear,
  onKeyDown,
}: DropdownSearchProps) {
  return (
    <div className="border-b border-[var(--border-clear)] px-2 py-2">
      <div className="flex items-center gap-2 rounded-lg bg-muted px-2.5 py-1.5 ring-inset focus-within:ring-2 focus-within:ring-brand/50">
        <Search size={11} className="shrink-0 text-foreground/30" />
        {/* The wrapper's `focus-within:ring` above is the single focus indicator.
            The global `:focus-visible { outline }` in utilities.css is UNLAYERED, so
            Tailwind's layered `focus-visible:outline-none` cannot override it under
            Tailwind v4 (layers beat specificity) — leaving the input with its own
            outline INSIDE the wrapper ring (a double ring). An inline `outline:none`
            outranks any author rule, so it reliably drops the input's outline. */}
        <input
          ref={searchRef}
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder={placeholder}
          style={{ outline: 'none' }}
          className="flex-1 bg-transparent text-xs text-foreground placeholder:text-foreground/25"
        />
        {onClear && search && (
          <button
            type="button"
            aria-label="Clear search"
            onClick={onClear}
            className="text-foreground/30 hover:text-foreground/60"
          >
            <X size={10} />
          </button>
        )}
      </div>
    </div>
  );
}
