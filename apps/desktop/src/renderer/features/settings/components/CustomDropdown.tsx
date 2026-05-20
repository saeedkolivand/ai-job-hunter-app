import { useState, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { motion, AnimatePresence } from 'motion/react';
import { transition } from '@/lib/motion';
import { Cpu, ChevronDown } from 'lucide-react';
import { cn } from '@/lib/cn';
import type { Model } from '@/types';
import { Button } from '@/components/ui/Button';

interface CustomDropdownProps {
  models: Model[];
  selectedModel: string;
  onSelectModel: (model: string) => void;
}

export function CustomDropdown({ models, selectedModel, onSelectModel }: CustomDropdownProps) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState({ top: 0, left: 0, width: 0 });
  const triggerRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open && triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      setPosition({
        top: rect.bottom + 8,
        left: rect.left,
        width: rect.width,
      });
    }
  }, [open]);

  const handleClickOutside = (e: MouseEvent) => {
    if (
      dropdownRef.current &&
      !dropdownRef.current.contains(e.target as Node) &&
      triggerRef.current &&
      !triggerRef.current.contains(e.target as Node)
    ) {
      setOpen(false);
    }
  };

  useEffect(() => {
    if (open) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [open]);

  return (
    <div ref={triggerRef}>
      <button
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
            {selectedModel || 'Select a model…'}
          </span>
        </div>
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
              ref={dropdownRef}
              initial={{ opacity: 0, y: -8, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -8, scale: 0.98 }}
              transition={transition.fast}
              style={{
                position: 'fixed',
                top: position.top,
                left: position.left,
                width: position.width,
                zIndex: 9999,
              }}
              className="overflow-hidden rounded-xl border border-white/10 bg-secondary shadow-xl"
            >
              <div className="max-h-64 overflow-y-auto px-1 py-1">
                {models.map((model) => (
                  <Button
                    key={model.name}
                    variant={selectedModel === model.name ? 'glass' : 'ghost'}
                    size="md"
                    onClick={() => {
                      onSelectModel(model.name);
                      setOpen(false);
                    }}
                    className={cn(
                      'w-full justify-between px-4 py-2.5',
                      selectedModel !== model.name && '!bg-transparent hover:bg-white/5'
                    )}
                  >
                    <span>{model.name}</span>
                    {model.size && <span className="text-xs text-foreground/40">{model.size}</span>}
                  </Button>
                ))}
              </div>
            </motion.div>
          </AnimatePresence>,
          document.body
        )}
    </div>
  );
}
