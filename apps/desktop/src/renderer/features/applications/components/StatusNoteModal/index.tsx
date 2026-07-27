import { useId, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, ModalShell, TextArea } from '@ajh/ui';

interface StatusNoteModalProps {
  open: boolean;
  /** Called for Skip / Escape / backdrop — the note is discarded. */
  onClose: () => void;
  /** The stage the note attaches to (already applied when `changed`). */
  status: string;
  /**
   * `true` when the modal follows a just-applied stage change (the copy then
   * reads "moved to X"); `false` for the Timeline's explicit "Add note".
   */
  changed?: boolean;
  isSaving?: boolean;
  /**
   * Localized message for a FAILED save. Non-null keeps the dialog open with the
   * typed note intact, so a rejected write never silently discards it.
   */
  error?: string | null;
  /**
   * Receives the trimmed note. Never called with an empty string. The CALLER
   * closes the dialog (by flipping `open`) once the write actually lands.
   */
  onSave: (note: string) => void;
}

/**
 * Optional-note prompt for an application status change.
 *
 * Shape decision: the stage change is applied IMMEDIATELY (never blocked on this
 * dialog) and the note is a follow-on — Skip/Escape is a no-op, so the common
 * "just move it to Applied" path costs one keystroke. Saving writes a SAME-status
 * `setStatus({ note })`, i.e. one extra append-only status event carrying the
 * note; the Timeline collapses a `from === to` event to a plain note entry.
 *
 * The dialog is CONTROLLED: it does not close itself on Save — the owner closes
 * it from the mutation's `onSuccess`, so a failed write keeps the text on screen.
 * The owner also lives above the invalidation refetch, which is why `open` is a
 * prop rather than local state.
 */
export function StatusNoteModal({
  open,
  onClose,
  status,
  changed = false,
  isSaving = false,
  error = null,
  onSave,
}: StatusNoteModalProps) {
  const { t } = useTranslation();
  const [note, setNote] = useState('');
  const titleId = useId();
  const fieldId = useId();

  // Clear the draft when the dialog OPENS (not when it closes): a fresh prompt
  // always starts empty, while a failed save — which keeps `open` true — keeps
  // the text the user just typed.
  const [seenOpen, setSeenOpen] = useState(open);
  if (seenOpen !== open) {
    setSeenOpen(open);
    if (open) setNote('');
  }

  const close = () => onClose();

  const save = () => {
    const trimmed = note.trim();
    if (!trimmed) return;
    onSave(trimmed);
  };

  const stageLabel = t(`applications.status.${status}` as const);

  return (
    <ModalShell
      open={open}
      onClose={close}
      maxWidth="max-w-md"
      ariaLabelledby={titleId}
      header={
        <div className="border-b border-[var(--border-soft)] px-5 py-4">
          <h2 id={titleId} className="text-sm font-semibold text-foreground/90">
            {t('applications.note.title')}
          </h2>
          <p className="mt-0.5 text-fine-print text-foreground/50">
            {changed
              ? t('applications.note.afterChange', { stage: stageLabel })
              : t('applications.note.current', { stage: stageLabel })}
          </p>
        </div>
      }
      footer={
        <div className="flex items-center justify-end gap-2 border-t border-[var(--border-soft)] px-5 py-3">
          <Button variant="ghost" onClick={close}>
            {t('applications.note.skip')}
          </Button>
          <Button
            variant="primary"
            onClick={save}
            disabled={note.trim().length === 0}
            loading={isSaving}
          >
            {t('applications.note.save')}
          </Button>
        </div>
      }
    >
      <div className="px-5 py-4">
        <label htmlFor={fieldId} className="sr-only">
          {t('applications.note.title')}
        </label>
        <TextArea
          id={fieldId}
          autoFocus
          rows={4}
          value={note}
          onChange={(e) => setNote(e.target.value)}
          placeholder={t('applications.note.placeholder')}
        />
        {error && (
          <p className="mt-2 text-fine-print text-destructive" role="alert">
            {error}
          </p>
        )}
      </div>
    </ModalShell>
  );
}
