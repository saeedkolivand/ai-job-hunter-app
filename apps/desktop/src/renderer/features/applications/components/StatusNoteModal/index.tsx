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
  /** Receives the trimmed note. Never called with an empty string. */
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
 */
export function StatusNoteModal({
  open,
  onClose,
  status,
  changed = false,
  isSaving = false,
  onSave,
}: StatusNoteModalProps) {
  const { t } = useTranslation();
  const [note, setNote] = useState('');
  const titleId = useId();
  const fieldId = useId();

  // Both exits clear the draft, so re-opening the modal never resurrects a note
  // the user walked away from (no effect needed — ModalShell keeps this mounted).
  const close = () => {
    setNote('');
    onClose();
  };

  const save = () => {
    const trimmed = note.trim();
    if (!trimmed) return;
    onSave(trimmed);
    close();
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
      </div>
    </ModalShell>
  );
}
