import { useRef, useState } from 'react';

import type { RewriteTarget } from '@/components/generation/EditableOutput/RewritePopover';
import { getSelectionOffsets } from '@/lib/selection-offsets';

/** The two independently-rewritable fields of the draft. */
export type EmailField = 'subject' | 'body';

/** A rewrite frozen at trigger time — which field, the splice range, the snapshot
 *  it splices back into on Accept, the rewrite target, and the anchor button. */
interface FrozenRewrite {
  field: EmailField;
  start: number;
  end: number;
  snapshot: string;
  target: RewriteTarget;
  anchorEl: HTMLElement;
}

/**
 * Select-to-rewrite (mirrors ApplicationQuestionsModal): one frozen rewrite at
 * a time. The selected span inside the field's <p> — or the whole field when
 * nothing is selected — is captured at trigger time and spliced back on Accept.
 * `onSplice` receives the field and its spliced text; the caller commits it.
 */
export function useEmailRewrite(
  draft: { subject: string; body: string },
  onSplice: (field: EmailField, spliced: string) => void
) {
  const subjectRef = useRef<HTMLParagraphElement | null>(null);
  const bodyRef = useRef<HTMLParagraphElement | null>(null);
  const [frozen, setFrozen] = useState<FrozenRewrite | null>(null);

  const openRewrite = (field: EmailField, trigger: HTMLElement) => {
    const text = draft[field];
    const container = field === 'subject' ? subjectRef.current : bodyRef.current;
    const offsets = container ? getSelectionOffsets(container) : null;
    const start = offsets?.start ?? 0;
    const end = offsets?.end ?? text.length;
    setFrozen({
      field,
      start,
      end,
      snapshot: text,
      anchorEl: trigger,
      target: {
        selection: text.slice(start, end),
        before: text.slice(0, start),
        after: text.slice(end),
      },
    });
  };

  const closeRewrite = () => {
    const trigger = frozen?.anchorEl;
    setFrozen(null);
    trigger?.focus();
  };

  // Splice the accepted replacement back into the frozen snapshot; the caller
  // commits it to the editable draft and persists it so the edit survives a tab
  // switch. Splicing into the snapshot also covers a rewrite of a draft that was
  // hydrated from the store and never re-generated.
  const acceptRewrite = (replacement: string) => {
    if (!frozen) return;
    const { field, start, end, snapshot } = frozen;
    setFrozen(null);
    onSplice(field, snapshot.slice(0, start) + replacement + snapshot.slice(end));
  };

  return { subjectRef, bodyRef, frozen, openRewrite, closeRewrite, acceptRewrite };
}

export type EmailRewrite = ReturnType<typeof useEmailRewrite>;
