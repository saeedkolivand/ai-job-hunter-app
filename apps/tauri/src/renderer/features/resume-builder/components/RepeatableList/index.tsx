import { type LucideIcon, Plus, Trash2 } from 'lucide-react';
import type { ReactNode } from 'react';

import { Button, EmptyState, GlassCard } from '@ajh/ui';

interface RepeatableListProps<T> {
  items: T[];
  onChange: (items: T[]) => void;
  /** Factory for a new blank entry. */
  blank: () => T;
  addLabel: string;
  removeLabel: string;
  /** Shown as the empty-state title when there are no entries yet. */
  emptyLabel: string;
  /** Optional empty-state supporting copy. */
  emptyDescription?: string;
  /** Icon for the empty state. */
  icon: LucideIcon;
  /** Render one entry's fields. `update` patches just this entry; `remove` drops it. */
  render: (item: T, update: (patch: Partial<T>) => void, index: number) => ReactNode;
}

/**
 * Generic add/remove/update list for the repeatable Resume Builder sections
 * (experience, education, projects, …). Treats `items` immutably so the session
 * slice's shared defaults are never mutated in place.
 */
export function RepeatableList<T>({
  items,
  onChange,
  blank,
  addLabel,
  removeLabel,
  emptyLabel,
  emptyDescription,
  icon,
  render,
}: RepeatableListProps<T>) {
  const add = () => onChange([...items, blank()]);
  const update = (index: number, patch: Partial<T>) =>
    onChange(items.map((item, i) => (i === index ? { ...item, ...patch } : item)));
  const remove = (index: number) => onChange(items.filter((_, i) => i !== index));

  const addButton = (
    <Button type="button" onClick={add} variant="ghost" size="sm" className="gap-1.5">
      <Plus size={14} />
      {addLabel}
    </Button>
  );

  if (items.length === 0) {
    return (
      <EmptyState
        icon={icon}
        title={emptyLabel}
        description={emptyDescription}
        action={addButton}
        className="py-10"
      />
    );
  }

  return (
    <div className="space-y-3">
      {items.map((item, index) => (
        // Index key is stable enough here: entries are only ever appended or
        // removed wholesale, never reordered.
        <GlassCard key={index} className="relative space-y-2.5 p-4">
          <Button
            type="button"
            onClick={() => remove(index)}
            variant="ghost"
            size="sm"
            aria-label={removeLabel}
            className="absolute right-2 top-2 text-foreground/40 hover:text-action-delete"
          >
            <Trash2 size={14} />
          </Button>
          {render(item, (patch) => update(index, patch), index)}
        </GlassCard>
      ))}

      {addButton}
    </div>
  );
}
