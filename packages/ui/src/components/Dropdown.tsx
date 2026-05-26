import { ChevronDown, Search } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { cn } from '../lib/cn';
import { transition } from '../lib/motion';
import { Button } from './Button';

export interface DropdownOption {
  value: string;
  label: string;
  /** Secondary text shown on the right (e.g. model size) */
  meta?: string;
  /** Section header for grouping options */
  section?: string;
}

export interface DropdownProps {
  options: DropdownOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  /** Leading icon in the trigger button */
  icon?: ReactNode;
  /** Show search box. Defaults to true when options > 5. */
  searchable?: boolean;
  disabled?: boolean;
}

export function Dropdown({
  options,
  value,
  onChange,
  placeholder = 'Select…',
  icon,
  searchable,
  disabled,
}: DropdownProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [position, setPosition] = useState({ top: 0, left: 0, width: 0 });
  const triggerRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const showSearch = searchable ?? options.length > 5;
  const selected = options.find((o) => o.value === value);

  const filtered = useMemo(() => {
    if (!query.trim()) return options;
    const q = query.toLowerCase();
    return options.filter((o) => o.label.toLowerCase().includes(q));
  }, [options, query]);

  // Group options by section
  const grouped = useMemo(() => {
    const groups: Record<string, DropdownOption[]> = {};
    filtered.forEach((opt) => {
      const section = opt.section || 'default';
      if (!groups[section]) groups[section] = [];
      groups[section].push(opt);
    });
    return groups;
  }, [filtered]);

  useEffect(() => {
    if (open && triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      setPosition({ top: rect.bottom + 6, left: rect.left, width: rect.width });
    }
    if (open) {
      setQuery('');
      setTimeout(() => searchRef.current?.focus(), 50);
    }
  }, [open]);

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

  return (
    <div ref={triggerRef}>
      <Button
        type="button"
        disabled={disabled}
        onClick={() => !disabled && setOpen((o) => !o)}
        className={cn(
          'glass-graphite glass-highlight flex h-9 w-full min-w-[200px] max-w-[400px] items-center justify-between gap-2 rounded-xl px-3 text-xs transition-all duration-150',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50',
          open ? 'border-brand/35' : 'hover:bg-white/[0.02]'
        )}
      >
        <div className="flex min-w-0 flex-1 items-center gap-2 overflow-hidden">
          {icon && <span className="shrink-0 text-foreground/40">{icon}</span>}
          <span
            className={cn(
              'truncate text-left',
              selected ? 'text-foreground/90' : 'text-foreground/35'
            )}
          >
            {selected?.label ?? placeholder}
          </span>
        </div>
        <ChevronDown
          size={12}
          className={cn(
            'shrink-0 text-foreground/30 transition-transform duration-150',
            open && 'rotate-180'
          )}
        />
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
              {showSearch && (
                <div className="border-b border-white/[0.06] px-3 py-2.5">
                  <div className="flex items-center gap-2 rounded-lg bg-white/[0.04] px-3 py-2">
                    <Search size={11} className="shrink-0 text-foreground/30" />
                    <input
                      ref={searchRef}
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      placeholder="Search…"
                      className="flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-foreground/25"
                    />
                  </div>
                </div>
              )}

              <div className="max-h-72 space-y-0.5 overflow-y-auto px-2 py-2">
                {filtered.length === 0 ? (
                  <div className="px-3 py-4 text-center text-xs text-foreground/35">No results</div>
                ) : (
                  Object.entries(grouped).map(([section, opts]) => (
                    <div key={section}>
                      {section !== 'default' && (
                        <div className="px-3 py-1.5 text-[10px] font-medium uppercase tracking-wider text-foreground/35">
                          {section}
                        </div>
                      )}
                      {opts.map((opt) => {
                        const isSelected = opt.value === value;
                        return (
                          <button
                            key={opt.value}
                            type="button"
                            onClick={() => {
                              onChange(opt.value);
                              setOpen(false);
                            }}
                            className={cn(
                              'flex w-full items-center justify-between rounded-lg px-3 py-2.5 text-xs transition-colors',
                              isSelected
                                ? 'bg-brand/15 text-brand-soft'
                                : 'text-foreground/70 hover:bg-white/[0.05] hover:text-foreground/90'
                            )}
                          >
                            <span className="truncate text-left">{opt.label}</span>
                            {opt.meta && (
                              <span className="ml-2 shrink-0 text-[10px] text-foreground/35">
                                {opt.meta}
                              </span>
                            )}
                          </button>
                        );
                      })}
                    </div>
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
