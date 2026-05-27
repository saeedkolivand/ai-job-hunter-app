import { useCallback } from 'react';

interface UseDropdownKeyboardProps {
  open: boolean;
  disabled: boolean;
  filtered: Array<{ value: string }>;
  value: string;
  highlighted: number;
  setOpen: (open: boolean) => void;
  setHighlighted: (index: number | ((prev: number) => number)) => void;
  select: (value: string) => void;
  triggerRef: React.RefObject<HTMLButtonElement | null>;
}

export function useDropdownKeyboard({
  open,
  disabled,
  filtered,
  value,
  highlighted,
  setOpen,
  setHighlighted,
  select,
  triggerRef,
}: UseDropdownKeyboardProps) {
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
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
    },
    [disabled, open, filtered, value, highlighted, setOpen, setHighlighted, select, triggerRef]
  );

  return handleKeyDown;
}
