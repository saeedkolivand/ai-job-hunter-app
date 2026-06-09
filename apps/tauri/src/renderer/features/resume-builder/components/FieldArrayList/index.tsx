import { type LucideIcon, Plus, Trash2 } from 'lucide-react';
import type { ReactNode } from 'react';

import { Button, EmptyState, GlassCard } from '@ajh/ui';

interface FieldArrayRow {
  /** react-hook-form's stable field id — used as the React key (never the index). */
  id: string;
}

interface FieldArrayListProps<T extends FieldArrayRow> {
  /** The `fields` array from `useFieldArray`. */
  fields: T[];
  /** Append a blank entry (caller supplies the blank shape). */
  onAppend: () => void;
  /** Remove the entry at `index`. */
  onRemove: (index: number) => void;
  addLabel: string;
  removeLabel: string;
  /** Empty-state title when there are no entries yet. */
  emptyLabel: string;
  emptyDescription?: string;
  icon: LucideIcon;
  /** Render one entry's fields, keyed by the field-array `index`. */
  render: (index: number) => ReactNode;
}

/**
 * Presentational add/remove list for the repeatable Resume Builder sections,
 * driven by a react-hook-form `useFieldArray`. Identical chrome to the former
 * `RepeatableList` (GlassCard + EmptyState + Add button + trash + the `pr-8`
 * overflow reservation) — only the data source changed from immutable props to
 * the field array, so per-field `Controller`s own their own state.
 */
export function FieldArrayList<T extends FieldArrayRow>({
  fields,
  onAppend,
  onRemove,
  addLabel,
  removeLabel,
  emptyLabel,
  emptyDescription,
  icon,
  render,
}: FieldArrayListProps<T>) {
  const addButton = (
    <Button type="button" onClick={onAppend} variant="ghost" size="sm" className="gap-1.5">
      <Plus size={14} />
      {addLabel}
    </Button>
  );

  if (fields.length === 0) {
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
      {fields.map((field, index) => (
        <GlassCard key={field.id} className="relative space-y-2.5 p-4">
          <Button
            type="button"
            onClick={() => onRemove(index)}
            variant="ghost"
            size="sm"
            aria-label={removeLabel}
            className="absolute right-2 top-2 text-foreground/40 hover:text-action-delete"
          >
            <Trash2 size={14} />
          </Button>
          {/* Reserve room for the absolute trash button so no field sits under it. */}
          <div className="pr-8">{render(index)}</div>
        </GlassCard>
      ))}

      {addButton}
    </div>
  );
}
