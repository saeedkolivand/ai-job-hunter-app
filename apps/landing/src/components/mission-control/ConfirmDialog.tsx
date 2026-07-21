'use client';

import { useEffect, useRef } from 'react';

// Accessible confirm gate for every write action. role="alertdialog" +
// aria-modal + labelled/described; focus lands on the confirm button on open,
// Escape cancels, Tab cycles between the two controls (a minimal focus trap),
// and focus is restored to the trigger on close. There is no click-to-confirm
// anywhere but this dialog.
export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = 'Confirm',
  danger = false,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const confirmRef = useRef<HTMLButtonElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const restoreRef = useRef<Element | null>(null);

  // Read onCancel through a ref so the trap effect can depend on [open] alone —
  // otherwise a fresh onCancel closure each parent render re-subscribes the
  // keydown listener and re-runs focus/restore while the dialog is open.
  const onCancelRef = useRef(onCancel);
  useEffect(() => {
    onCancelRef.current = onCancel;
  }, [onCancel]);

  useEffect(() => {
    if (!open) return;
    restoreRef.current = document.activeElement;
    confirmRef.current?.focus();

    // Scroll-lock the page behind the modal; restore the prior value on close.
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    function onKey(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        event.preventDefault();
        onCancelRef.current();
        return;
      }
      if (event.key === 'Tab') {
        // Two focusable controls — keep focus inside the dialog.
        const active = document.activeElement;
        event.preventDefault();
        if (event.shiftKey) {
          (active === cancelRef.current ? confirmRef : cancelRef).current?.focus();
        } else {
          (active === confirmRef.current ? cancelRef : confirmRef).current?.focus();
        }
      }
    }
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.body.style.overflow = prevOverflow;
      if (restoreRef.current instanceof HTMLElement) restoreRef.current.focus();
    };
  }, [open]);

  if (!open) return null;

  return (
    <div className="mc-overlay">
      <div
        className="mc-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="mc-dialog-title"
        aria-describedby="mc-dialog-msg"
      >
        <h2 className="mc-dialog__title" id="mc-dialog-title">
          {title}
        </h2>
        <p className="mc-dialog__msg" id="mc-dialog-msg">
          {message}
        </p>
        <div className="mc-dialog__actions">
          <button type="button" ref={cancelRef} className="mc-btn" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            ref={confirmRef}
            className={danger ? 'mc-btn is-danger' : 'mc-btn is-primary'}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
