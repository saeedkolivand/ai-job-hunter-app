import { ChevronDown, Cpu, Search } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { Button } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { transition } from '@/lib/motion';
import type { Model } from '@/types';

interface CustomDropdownProps {
  models: Model[];
  selectedModel: string;
  onSelectModel: (model: string) => void;
  /** Show a search box inside the dropdown. Defaults to true when models > 5. */
  searchable?: boolean;
  placeholder?: string;
}

export function CustomDropdown({
  models,
  selectedModel,
  onSelectModel,
  searchable,
  placeholder = 'Select a model…',
}: CustomDropdownProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [position, setPosition] = useState({ top: 0, left: 0, width: 0 });
  const triggerRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const showSearch = searchable ?? models.length > 5;

  const filtered = useMemo(() => {
    if (!query.trim()) return models;
    const q = query.toLowerCase();
    return models.filter((m) => m.name.toLowerCase().includes(q));
  }, [models, query]);

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

  const handleSelect = (name: string) => {
    onSelectModel(name);
    setOpen(false);
  };

  return (
    <div ref={triggerRef}>
      <Button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className={cn(
          'glass-graphite glass-highlight flex h-9 w-full items-center justify-between gap-2 rounded-xl px-3 text-xs transition-all duration-150',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50',
          open ? 'border-brand/35' : 'hover:bg-white/[0.02]'
        )}
      >
        <div className="flex items-center gap-2">
          <Cpu size={13} className="shrink-0 text-foreground/40" />
          <span
            className={cn('truncate', selectedModel ? 'text-foreground/90' : 'text-foreground/35')}
          >
            {selectedModel || placeholder}
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

      {open &&
        createPortal(
          <AnimatePresence>
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
                <div className="border-b border-white/[0.06] px-2 py-2">
                  <div className="flex items-center gap-2 rounded-lg bg-white/[0.04] px-2.5 py-1.5">
                    <Search size={11} className="shrink-0 text-foreground/30" />
                    <input
                      ref={searchRef}
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      placeholder="Search models…"
                      className="flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-foreground/25"
                    />
                  </div>
                </div>
              )}

              <div className="max-h-56 overflow-y-auto px-1 py-1 space-y-1">
                {filtered.length === 0 ? (
                  <div className="px-3 py-4 text-center text-xs text-foreground/35">
                    No models match
                  </div>
                ) : (
                  filtered.map((model) => {
                    const isSelected = selectedModel === model.name;
                    return (
                      <button
                        key={model.name}
                        onClick={() => handleSelect(model.name)}
                        className={cn(
                          'flex w-full items-center justify-between rounded-lg px-3 py-2 text-xs transition-colors',
                          isSelected
                            ? 'bg-brand/15 text-brand-soft'
                            : 'text-foreground/70 hover:bg-white/[0.05] hover:text-foreground/90'
                        )}
                      >
                        <span>{model.name}</span>
                        {model.size && (
                          <span className="text-[10px] text-foreground/35">{model.size}</span>
                        )}
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
