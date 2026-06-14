import { Check, ChevronDown } from 'lucide-react';
import { type ReactNode, useEffect, useId, useMemo, useRef, useState } from 'react';

import { useDropdownKeyboard } from '../../hooks/useDropdownKeyboard';
import { useDropdownPosition } from '../../hooks/useDropdownPosition';
import { cn } from '../../lib/cn';
import { DropdownPanel } from '../DropdownPanel';
import { DropdownSearch } from '../DropdownSearch';

const SEARCHABLE_THRESHOLD = 8;

export interface DropdownOption {
  value: string;
  label: string;
  /** Leading icon shown before the label. */
  icon?: ReactNode;
  /** Secondary text shown on the right (e.g. model size). */
  meta?: string;
  /** Section header for grouping options (visual only — keyboard nav stays flat). */
  section?: string;
}

export interface DropdownProps {
  options: DropdownOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  /** Leading icon in the trigger button. */
  icon?: ReactNode;
  /** Forwarded to the trigger button so an external `<label htmlFor>` can name the control. */
  id?: string;
  /** Override auto-search detection. Defaults to true when options.length >= 8. */
  searchable?: boolean;
  /** Tailwind max-height class applied to the options list. Defaults to 'max-h-56'. */
  listClassName?: string;
  /** Trigger height — 'sm' matches Button size="sm" (h-7); 'md' is the default (h-9). */
  size?: 'sm' | 'md';
  /**
   * Trigger accent. `default` is the neutral glass trigger; `primary` tints the
   * trigger with the brand colour (e.g. an application-status selector). Opt-in
   * per call site — other dropdowns stay neutral.
   */
  tone?: 'default' | 'primary';
}

export function Dropdown({
  options,
  value,
  onChange,
  placeholder = 'Select…',
  disabled,
  icon,
  id: idProp,
  searchable,
  listClassName,
  size = 'md',
  tone = 'default',
}: DropdownProps) {
  const generatedId = useId();
  const id = idProp ?? generatedId;
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [highlighted, setHighlighted] = useState<number>(-1);

  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const selectedOption = options.find((o) => o.value === value);
  const showSearch = searchable ?? options.length >= SEARCHABLE_THRESHOLD;

  const filtered = useMemo(
    () =>
      showSearch && search.trim()
        ? options.filter((o) => o.label.toLowerCase().includes(search.toLowerCase()))
        : options,
    [showSearch, search, options]
  );

  // Group filtered options by section, preserving order. Keyboard nav still
  // operates over the flat `filtered` array — sections are visual only, so we
  // track each option's flat index for `data-idx`/highlight wiring.
  const groups = useMemo(() => {
    const out: Array<{ section: string; items: Array<{ option: DropdownOption; index: number }> }> =
      [];
    filtered.forEach((option, index) => {
      const section = option.section || 'default';
      const last = out[out.length - 1];
      if (last && last.section === section) {
        last.items.push({ option, index });
      } else {
        out.push({ section, items: [{ option, index }] });
      }
    });
    return out;
  }, [filtered]);

  const { dropUp, dropdownStyle } = useDropdownPosition(open, triggerRef);

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

  const select = (val: string) => {
    onChange(val);
    setOpen(false);
    setSearch('');
    triggerRef.current?.focus();
  };

  const handleKeyDown = useDropdownKeyboard({
    open,
    disabled: disabled ?? false,
    filtered,
    value,
    highlighted,
    setOpen,
    setHighlighted,
    select,
    triggerRef,
  });

  // Scroll highlighted item into view
  useEffect(() => {
    if (highlighted < 0) return;
    listRef.current
      ?.querySelector(`[data-idx="${highlighted}"]`)
      ?.scrollIntoView({ block: 'nearest' });
  }, [highlighted]);

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
          'flex w-full items-center justify-between gap-2 rounded-lg text-xs transition-all duration-150 shadow-sm',
          size === 'sm' ? 'h-7 px-2.5' : 'h-9 px-3',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1 focus-visible:ring-offset-transparent',
          'disabled:cursor-not-allowed disabled:opacity-40',
          tone === 'primary'
            ? cn(
                'border border-brand/30 bg-brand/10 text-brand-soft',
                open ? 'border-brand/55 bg-brand/15' : 'hover:border-brand/45 hover:bg-brand/15'
              )
            : cn(
                'border border-[var(--border-clear)] bg-card',
                open
                  ? 'border-brand/45 bg-muted text-foreground/90'
                  : 'text-foreground/75 hover:border-[var(--border-clear)] hover:bg-muted hover:text-foreground/90'
              )
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
            'shrink-0 transition-transform duration-150',
            tone === 'primary' ? 'text-brand-soft/70' : 'text-foreground/30',
            open && 'rotate-180'
          )}
        />
      </button>

      <DropdownPanel
        open={open}
        style={dropdownStyle}
        dropUp={dropUp}
        panelRef={listRef}
        role="listbox"
        {...(idProp ? { 'aria-labelledby': id } : { 'aria-label': placeholder })}
        onKeyDown={handleKeyDown}
      >
        {showSearch && (
          <DropdownSearch search={search} setSearch={setSearch} searchRef={searchRef} />
        )}

        <div className={cn('overflow-y-auto p-1 scrollbar-thin', listClassName ?? 'max-h-56')}>
          {filtered.length === 0 ? (
            <div className="px-3 py-6 text-center text-[11px] text-foreground/30">No results</div>
          ) : (
            groups.map((group) => (
              <div key={group.section}>
                {group.section !== 'default' && (
                  <div className="px-3 py-1.5 text-[10px] font-medium uppercase tracking-wider text-foreground/35">
                    {group.section}
                  </div>
                )}
                {group.items.map(({ option, index }) => {
                  const isSelected = option.value === value;
                  const isHighlighted = index === highlighted;
                  return (
                    <div
                      key={option.value}
                      role="option"
                      tabIndex={-1}
                      aria-selected={isSelected}
                      data-idx={index}
                      onMouseEnter={() => setHighlighted(index)}
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
                      <span className="flex shrink-0 items-center gap-2">
                        {option.meta && (
                          <span className="text-[10px] text-foreground/35">{option.meta}</span>
                        )}
                        {isSelected && <Check size={11} className="shrink-0 text-brand-soft" />}
                      </span>
                    </div>
                  );
                })}
              </div>
            ))
          )}
        </div>
      </DropdownPanel>
    </div>
  );
}
