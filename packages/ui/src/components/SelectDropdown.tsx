import { Check, ChevronDown, Search } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { cn } from '../lib/cn';

const SEARCHABLE_THRESHOLD = 8;

interface SelectOption {
  value: string;
  label: string;
  icon?: React.ReactNode;
}

interface SelectDropdownProps {
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  icon?: React.ReactNode;
}

export function SelectDropdown({
  options,
  value,
  onChange,
  placeholder = 'Select…',
  disabled,
  icon,
}: SelectDropdownProps) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [highlighted, setHighlighted] = useState<number>(-1);
  const [rect, setRect] = useState<DOMRect | null>(null);

  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const selectedOption = options.find((o) => o.value === value);
  const showSearch = options.length >= SEARCHABLE_THRESHOLD;

  const filtered =
    showSearch && search.trim()
      ? options.filter((o) => o.label.toLowerCase().includes(search.toLowerCase()))
      : options;

  // ── positioning ──────────────────────────────────────────────────────────
  const measure = useCallback(() => {
    if (triggerRef.current) setRect(triggerRef.current.getBoundingClientRect());
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

  // Flip up if not enough space below
  const spaceBelow = rect ? window.innerHeight - rect.bottom : 999;
  const dropUp = spaceBelow < 220;

  // ── outside click ────────────────────────────────────────────────────────
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!triggerRef.current?.contains(target) && !listRef.current?.contains(target)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  // ── keyboard navigation ──────────────────────────────────────────────────
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (disabled) return;
    if (!open) {
      if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown') {
        e.preventDefault();
        setOpen(true);
        setHighlighted(filtered.findIndex((o) => o.value === value));
      }
      return;
    }
    if (e.key === 'Escape') {
      setOpen(false);
      triggerRef.current?.focus();
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlighted((h) => Math.min(h + 1, filtered.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlighted((h) => Math.max(h - 1, 0));
    } else if (e.key === 'Enter' && highlighted >= 0 && filtered[highlighted]) {
      e.preventDefault();
      select(filtered[highlighted].value);
    } else if (e.key === 'Tab') {
      setOpen(false);
    }
  };

  const select = (val: string) => {
    onChange(val);
    setOpen(false);
    setSearch('');
    triggerRef.current?.focus();
  };

  // Scroll highlighted item into view
  useEffect(() => {
    if (highlighted < 0) return;
    listRef.current
      ?.querySelector(`[data-idx="${highlighted}"]`)
      ?.scrollIntoView({ block: 'nearest' });
  }, [highlighted]);

  // Refs for values that should be readable in effects without re-triggering them
  const filteredRef = useRef(filtered);
  const valueRef = useRef(value);
  const showSearchRef = useRef(showSearch);
  filteredRef.current = filtered;
  valueRef.current = value;
  showSearchRef.current = showSearch;

  // Focus search when opened; sync highlighted item to current value
  useEffect(() => {
    if (open && showSearchRef.current) {
      setTimeout(() => searchRef.current?.focus(), 50);
    }
    if (open) {
      setHighlighted(filteredRef.current.findIndex((o) => o.value === valueRef.current));
    }
  }, [open]);

  // Reset highlight to first item when the search query changes
  useEffect(() => {
    setHighlighted(filteredRef.current.length > 0 ? 0 : -1);
  }, [search]);

  const dropdownStyle: React.CSSProperties = rect
    ? {
        position: 'fixed',
        left: rect.left,
        width: rect.width,
        zIndex: 9999,
        ...(dropUp ? { bottom: window.innerHeight - rect.top + 4 } : { top: rect.bottom + 4 }),
      }
    : { display: 'none' };

  return (
    <div className="relative">
      <button
        ref={triggerRef}
        id={id}
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => {
          if (!disabled) {
            setOpen((o) => !o);
          }
        }}
        onKeyDown={handleKeyDown}
        className={cn(
          'flex h-8 w-full items-center justify-between gap-2 rounded-lg px-3 text-xs transition-all duration-150',
          'border border-white/[0.06] bg-white/[0.03]',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1 focus-visible:ring-offset-transparent',
          'disabled:cursor-not-allowed disabled:opacity-40',
          open
            ? 'border-brand/35 bg-white/[0.05] text-foreground/90'
            : 'text-foreground/70 hover:border-white/10 hover:bg-white/[0.05] hover:text-foreground/90'
        )}
      >
        <span className="flex min-w-0 items-center gap-2">
          {icon && <span className="shrink-0 text-foreground/40">{icon}</span>}
          <span className={cn('truncate', !selectedOption && 'text-foreground/35')}>
            {selectedOption?.label ?? placeholder}
          </span>
        </span>
        <ChevronDown
          size={12}
          className={cn(
            'shrink-0 text-foreground/30 transition-transform duration-150',
            open && 'rotate-180'
          )}
        />
      </button>

      {open &&
        createPortal(
          <AnimatePresence>
            <motion.div
              key="dropdown"
              ref={listRef}
              role="listbox"
              aria-labelledby={id}
              initial={{ opacity: 0, y: dropUp ? 4 : -4, scale: 0.985 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: dropUp ? 4 : -4, scale: 0.985 }}
              transition={{ duration: 0.13, ease: [0.22, 1, 0.36, 1] }}
              style={dropdownStyle}
              onKeyDown={handleKeyDown}
              className="overflow-hidden rounded-xl border border-white/8 bg-black/60 shadow-2xl shadow-black/60 backdrop-blur-xl"
            >
              {/* Search */}
              {showSearch && (
                <div className="border-b border-white/[0.06] px-2 py-2">
                  <div className="flex items-center gap-2 rounded-lg bg-white/[0.04] px-2.5 py-1.5">
                    <Search size={11} className="shrink-0 text-foreground/30" />
                    <input
                      ref={searchRef}
                      type="text"
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                      placeholder="Search…"
                      className="flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-foreground/25"
                    />
                  </div>
                </div>
              )}

              {/* Options */}
              <div className="max-h-56 overflow-y-auto p-1 scrollbar-thin">
                {filtered.length === 0 ? (
                  <div className="px-3 py-6 text-center text-[11px] text-foreground/30">
                    No results
                  </div>
                ) : (
                  filtered.map((option, idx) => {
                    const isSelected = option.value === value;
                    const isHighlighted = idx === highlighted;
                    return (
                      <button
                        key={option.value}
                        type="button"
                        role="option"
                        aria-selected={isSelected}
                        data-idx={idx}
                        onMouseEnter={() => setHighlighted(idx)}
                        onClick={() => select(option.value)}
                        className={cn(
                          'flex w-full items-center justify-between gap-2 rounded-lg px-3 py-2 text-xs transition-colors duration-100',
                          isHighlighted && !isSelected && 'bg-white/[0.05] text-foreground/90',
                          isSelected ? 'bg-brand/15 text-brand-soft' : 'text-foreground/65'
                        )}
                      >
                        <span className="flex min-w-0 items-center gap-2">
                          {option.icon && <span className="shrink-0">{option.icon}</span>}
                          <span className="truncate">{option.label}</span>
                        </span>
                        {isSelected && <Check size={11} className="shrink-0 text-brand-soft" />}
                      </button>
                    );
                  })
                )}
              </div>
            </motion.div>
          </AnimatePresence>,
          document.body
        )}
    </div>
  );
}
