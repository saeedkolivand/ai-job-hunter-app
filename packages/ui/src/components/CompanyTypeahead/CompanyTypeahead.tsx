import { Plus, Search, Sparkles, Star, X } from 'lucide-react';
import {
  type KeyboardEvent,
  type ReactNode,
  type Ref,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from 'react';

import { cn } from '../../lib/cn';

/**
 * One row in the typeahead — a passively-discovered or curated-seed ATS company.
 * `atsKind`/`slug`/`displayName` originate from scraped content, so they are
 * ONLY ever rendered as JSX text nodes here (never linkified / dangerouslySet…)
 * and NEVER used as a dynamic object-property key (ambiguity is counted with a
 * `Map`, list keys are string templates) — see the Feature-B security advisory.
 */
export interface CompanyOption {
  /** Registry board id (`greenhouse`, `lever`, `ashby`, …). */
  atsKind: string;
  /** Company slug — casing preserved (Ashby tokens are case-sensitive). */
  slug: string;
  /** Backfilled from a posting's company name, when known. */
  displayName?: string;
  /** How many postings this slug has been seen in (0 for an unseen curated seed). */
  seenCount: number;
  /** Whether the user has starred it (a "watched company"). */
  starred: boolean;
  /** True for a curated-seed row that has never been organically discovered. */
  curated?: boolean;
}

/**
 * Imperative handle for a blur-independent flush. `onBlur` add-on-blur is not
 * reliable on every engine (WebKit WKWebView / WebKitGTK don't consistently
 * blur a focused input when a sibling button is clicked), so a submit path must
 * be able to flush a typed-but-unentered slug deterministically.
 */
export interface CompanyTypeaheadHandle {
  /** Synchronously add a non-empty pending query as a chip; idempotent once cleared. */
  commitPending: () => void;
}

export interface CompanyTypeaheadProps {
  /** Imperative handle exposing {@link CompanyTypeaheadHandle.commitPending}. */
  ref?: Ref<CompanyTypeaheadHandle>;
  /** Currently-selected company slugs — rendered as removable chips. */
  selected: string[];
  onAdd: (slug: string) => void;
  onRemove: (slug: string) => void;
  /** Controlled query text (parent debounces it into its suggestion fetch). */
  query: string;
  onQueryChange: (query: string) => void;
  /** Already-fetched + merged suggestions (discovered rows first, curated after). */
  suggestions: CompanyOption[];
  /** Toggle the star (watched) state of a suggestion row. */
  onToggleStar: (option: CompanyOption) => void;
  loading?: boolean;
  disabled?: boolean;
  placeholder?: string;
  /**
   * Accessible name for a row's star toggle — a FUNCTION so each row names its
   * own company (rotor/voice-control users would otherwise meet N identical
   * "Watch company" controls). State is conveyed separately by `aria-pressed`.
   */
  starLabel: (option: CompanyOption) => string;
  /** Accessible name for a chip's remove button, per-company (see `starLabel`). */
  removeLabel: (slug: string) => string;
  /** Subtle marker text shown on a curated-seed row. */
  curatedLabel?: string;
  /**
   * Visually-hidden `aria-live` text announcing the current result `count` while
   * the panel is open — the results feedback a screen-reader user would get from
   * a listbox popup, without reintroducing `role="option"` (Feature-A lesson).
   */
  resultsLabel?: (count: number) => string;
  /** Rendered inside the open panel when there are no suggestions (e.g. a hint). */
  emptyState?: ReactNode;
  className?: string;
  id?: string;
  inputTestId?: string;
  suggestionTestId?: string;
  starTestId?: string;
  chipTestId?: string;
}

/**
 * Multi-select company-slug typeahead modeled on {@link LocationInput}'s
 * fetch + keyboard-nav conventions, but ADD-to-list: selected slugs become
 * removable chips feeding a `string[]`, and free-text is always addable so an
 * unknown slug is never a dead end.
 *
 * **A11y (APG):** the result rows are NOT `role="option"` and there is no
 * `aria-activedescendant`. Arrow keys move a purely-visual highlight and Enter
 * commits it; each row is a plain pair of sibling buttons (add + star), so a
 * screen-reader/keyboard user can Tab straight to the star. This deliberately
 * avoids the "interactive child inside a role=option" violation (Feature-A
 * listbox lesson) — a listbox option must not contain its own button.
 */
export function CompanyTypeahead({
  ref,
  selected,
  onAdd,
  onRemove,
  query,
  onQueryChange,
  suggestions,
  onToggleStar,
  loading = false,
  disabled = false,
  placeholder,
  starLabel,
  removeLabel,
  curatedLabel,
  resultsLabel,
  emptyState,
  className,
  id,
  inputTestId,
  suggestionTestId,
  starTestId,
  chipTestId,
}: CompanyTypeaheadProps) {
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Count how many suggestions share a slug so we only surface the ATS-kind
  // badge when it disambiguates (same slug across e.g. greenhouse + lever). A
  // Map keyed by the lowercased slug — never a dynamic object key on scraped data.
  const slugCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const s of suggestions) {
      const key = s.slug.toLowerCase();
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return counts;
  }, [suggestions]);

  // Add a slug to the list and clear the query. `add` alone is used by
  // blur-commit (focus is intentionally leaving); `commit` also refocuses the
  // input for the stay-in-place paths (Enter / suggestion click).
  const add = (raw: string) => {
    const value = raw.trim();
    if (!value) return;
    onAdd(value);
    onQueryChange('');
    setActiveIndex(-1);
  };
  const commit = (raw: string) => {
    add(raw);
    inputRef.current?.focus();
  };

  // Blur-independent flush for the submit path (see CompanyTypeaheadHandle). No
  // deps array so the closure always reads the latest `query`; once `add` clears
  // the query a repeat call is a no-op, so it's safe alongside the blur-commit.
  useImperativeHandle(ref, () => ({
    commitPending() {
      if (query.trim()) add(query);
    },
  }));

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, suggestions.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, -1));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const active = activeIndex >= 0 ? suggestions[activeIndex] : undefined;
      if (active) commit(active.slug);
      else if (query.trim()) commit(query);
    } else if (e.key === 'Escape') {
      setOpen(false);
      setActiveIndex(-1);
    } else if (e.key === 'Backspace' && query === '' && selected.length > 0) {
      // Tag-input convention: backspace on an empty field removes the last chip.
      const last = selected[selected.length - 1];
      if (last !== undefined) onRemove(last);
    }
  };

  return (
    <div
      ref={containerRef}
      className={className}
      onFocus={() => !disabled && setOpen(true)}
      onBlur={(e) => {
        // Keep the panel open while focus stays inside the widget (tabbing to a
        // row's star button, or a mid-click on a suggestion/chip).
        if (containerRef.current?.contains(e.relatedTarget as Node | null)) return;
        // Focus truly left: flush a typed-but-uncommitted slug so Start Scrape
        // can't silently drop it (mirrors the free-text Enter path).
        if (query.trim()) add(query);
        setOpen(false);
        setActiveIndex(-1);
      }}
    >
      {/* Field: chips + the typing input, styled like an @ajh/ui Input. */}
      <div
        className={cn(
          'flex min-h-9 w-full flex-wrap items-center gap-1.5 rounded-lg bg-field px-2.5 py-1.5',
          'border border-[var(--border-clear)] transition-colors',
          'focus-within:ring-2 focus-within:ring-brand/50',
          disabled && 'opacity-50'
        )}
      >
        <Search size={13} className="shrink-0 text-foreground/35" aria-hidden="true" />
        {selected.map((slug) => (
          <span
            key={slug}
            data-testid={chipTestId}
            className="inline-flex items-center gap-1 rounded-md bg-brand/15 py-0.5 pl-2 pr-1 text-[11px] text-brand-soft"
          >
            <span className="max-w-[12rem] truncate">{slug}</span>
            <button
              type="button"
              aria-label={removeLabel(slug)}
              disabled={disabled}
              onClick={() => onRemove(slug)}
              className="rounded p-0.5 text-brand-soft/70 hover:bg-brand/20 hover:text-brand-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50"
            >
              <X size={11} aria-hidden="true" />
            </button>
          </span>
        ))}
        <input
          ref={inputRef}
          id={id}
          type="text"
          data-testid={inputTestId}
          value={query}
          disabled={disabled}
          placeholder={selected.length === 0 ? placeholder : undefined}
          autoComplete="off"
          onChange={(e) => {
            onQueryChange(e.target.value);
            setOpen(true);
            setActiveIndex(-1);
          }}
          onKeyDown={handleKeyDown}
          style={{ outline: 'none' }}
          className="min-w-[6rem] flex-1 bg-transparent text-sm text-foreground placeholder:text-foreground/25"
        />
      </div>

      {/* Results panel — inline (no portal) so it can't be obscured by chrome
          and needs no positioning math. Rows are plain buttons, not options. */}
      {open && !disabled && (
        <div className="dropdown-surface mt-1.5 overflow-hidden rounded-lg">
          <div className="max-h-56 space-y-0.5 overflow-y-auto p-1 scrollbar-thin">
            {suggestions.map((s, i) => {
              const rowKey = `${s.atsKind}:${s.slug}`;
              const showKind = (slugCounts.get(s.slug.toLowerCase()) ?? 0) > 1;
              return (
                <div
                  key={rowKey}
                  data-testid={suggestionTestId}
                  onMouseEnter={() => setActiveIndex(i)}
                  className={cn(
                    'flex items-center gap-1 rounded-md pr-1 transition-colors',
                    i === activeIndex ? 'bg-brand/15' : 'hover:bg-muted'
                  )}
                >
                  <button
                    type="button"
                    disabled={disabled}
                    onClick={() => commit(s.slug)}
                    className="flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50"
                  >
                    <Plus size={11} className="shrink-0 text-foreground/35" aria-hidden="true" />
                    <span className="min-w-0 flex-1 truncate text-foreground/85">
                      {s.displayName || s.slug}
                    </span>
                    {showKind && (
                      <span className="shrink-0 rounded bg-muted px-1 py-px text-[9px] uppercase tracking-wide text-foreground/45">
                        {s.atsKind}
                      </span>
                    )}
                    {s.curated && curatedLabel && (
                      <span className="inline-flex shrink-0 items-center gap-0.5 text-[9px] text-foreground/40">
                        <Sparkles size={9} aria-hidden="true" />
                        {curatedLabel}
                      </span>
                    )}
                    {s.seenCount > 0 && (
                      <span className="shrink-0 text-[10px] tabular-nums text-foreground/30">
                        {s.seenCount}
                      </span>
                    )}
                  </button>
                  <button
                    type="button"
                    data-testid={starTestId}
                    aria-label={starLabel(s)}
                    aria-pressed={s.starred}
                    disabled={disabled}
                    onClick={() => onToggleStar(s)}
                    className={cn(
                      'shrink-0 rounded p-1 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50',
                      s.starred ? 'text-amber-400' : 'text-foreground/30 hover:text-foreground/60'
                    )}
                  >
                    <Star size={13} aria-hidden="true" fill={s.starred ? 'currentColor' : 'none'} />
                  </button>
                </div>
              );
            })}
            {suggestions.length === 0 && !loading && emptyState}
            {loading && suggestions.length === 0 && (
              <div className="px-3 py-3 text-center text-[11px] text-foreground/25">…</div>
            )}
          </div>
        </div>
      )}

      {/* Result-count feedback for assistive tech — a persistent live region (so
          updates are announced) in place of a listbox popup's implicit count.
          Only speaks during an active query so an empty focus stays quiet. */}
      {resultsLabel && (
        <div className="sr-only" role="status" aria-live="polite">
          {open && query.trim() ? resultsLabel(suggestions.length) : ''}
        </div>
      )}
    </div>
  );
}
