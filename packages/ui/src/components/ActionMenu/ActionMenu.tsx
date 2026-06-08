import { MoreVertical } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, type RefObject, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { useFocusTrap } from '../../hooks/use-focus-trap';
import { cn } from '../../lib/cn';
import { transition } from '../../lib/motion';
import { Button } from '../Button';

export interface ActionMenuItem {
  label: string;
  onSelect: () => void;
  /** Leading icon. */
  icon?: ReactNode;
  /** Renders the item in the destructive (delete) colour. */
  destructive?: boolean;
  disabled?: boolean;
}

export interface ActionMenuProps {
  items: ActionMenuItem[];
  /** Accessible label for the trigger button. */
  label?: string;
  /** Which edge of the menu aligns to the trigger. Default `end` (right). */
  align?: 'start' | 'end';
  className?: string;
}

const MENU_WIDTH = 200;

/**
 * Overflow "3-dots" action menu (#32, #46). A small icon trigger opens a
 * portalled menu of actions; destructive items (e.g. Delete) take the
 * `action-delete` token colour. Closes on select, click-outside, or Escape;
 * focus is trapped while open (first item auto-focused).
 */
export function ActionMenu({
  items,
  label = 'Actions',
  align = 'end',
  className,
}: ActionMenuProps) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState({ top: 0, left: 0 });
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useFocusTrap(open);

  useEffect(() => {
    if (!open || !triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    const margin = 8;
    const left =
      align === 'end'
        ? Math.max(margin, rect.right - MENU_WIDTH)
        : Math.min(rect.left, window.innerWidth - MENU_WIDTH - margin);
    setPosition({ top: rect.bottom + 6, left });
  }, [open, align]);

  useEffect(() => {
    if (!open) return;
    const onPointer = (e: MouseEvent) => {
      if (
        menuRef.current?.contains(e.target as Node) ||
        triggerRef.current?.contains(e.target as Node)
      )
        return;
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener('mousedown', onPointer);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onPointer);
      document.removeEventListener('keydown', onKey);
    };
  }, [open, menuRef]);

  return (
    <>
      <Button
        ref={triggerRef}
        type="button"
        variant="ghost"
        size="sm"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={label}
        onClick={() => setOpen((o) => !o)}
        className={cn('aspect-square px-0', className)}
      >
        <MoreVertical size={16} />
      </Button>

      {createPortal(
        <AnimatePresence>
          {open && (
            <motion.div
              ref={menuRef as RefObject<HTMLDivElement>}
              role="menu"
              aria-label={label}
              initial={{ opacity: 0, y: -6, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -6, scale: 0.98 }}
              transition={transition.fast}
              style={{
                position: 'fixed',
                top: position.top,
                left: position.left,
                width: MENU_WIDTH,
                zIndex: 9999,
              }}
              className="dropdown-surface overflow-hidden rounded-xl p-1.5"
            >
              {items.map((item) => (
                <button
                  key={item.label}
                  type="button"
                  role="menuitem"
                  disabled={item.disabled}
                  onClick={() => {
                    item.onSelect();
                    setOpen(false);
                  }}
                  className={cn(
                    'flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-left text-caption transition-colors',
                    'disabled:pointer-events-none disabled:opacity-45',
                    item.destructive
                      ? 'text-action-delete hover:bg-action-delete/10'
                      : 'text-foreground/75 hover:bg-white/[0.06] hover:text-foreground'
                  )}
                >
                  {item.icon && <span className="shrink-0">{item.icon}</span>}
                  <span className="truncate">{item.label}</span>
                </button>
              ))}
            </motion.div>
          )}
        </AnimatePresence>,
        document.body
      )}
    </>
  );
}
