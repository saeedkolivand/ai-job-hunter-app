import { Contact } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, ModalShell } from '@ajh/ui';

import { ContactProfileForm } from '@/components/contact/ContactProfileForm';

interface ContactPromptModalProps {
  open: boolean;
  /** Dismiss without proceeding (Escape / backdrop / Cancel). */
  onClose: () => void;
  /** Proceed with the gated action (e.g. start generation). */
  onContinue: () => void;
}

/**
 * One-time pre-generation prompt: before the user's first résumé / cover-letter
 * generation we surface the contact profile so the document header is complete.
 * Reuses the same [`ContactProfileForm`] as Settings, so the fields never drift.
 * Whether this appears at all is gated by the persisted `contactPromptSeen` flag.
 */
export function ContactPromptModal({ open, onClose, onContinue }: ContactPromptModalProps) {
  const { t } = useTranslation();

  return (
    <ModalShell open={open} onClose={onClose} maxWidth="max-w-2xl">
      <div className="flex max-h-[85vh] flex-col">
        <div className="flex items-start gap-2 border-b border-foreground/10 px-6 py-5">
          <Contact size={16} className="mt-0.5 shrink-0 text-brand-soft" />
          <div className="flex flex-col gap-1">
            <span className="text-sm font-semibold text-foreground/85">
              {t('contactPrompt.title')}
            </span>
            <p className="text-xs text-foreground/55">{t('contactPrompt.description')}</p>
          </div>
        </div>

        <div className="overflow-y-auto px-6 py-5">
          <ContactProfileForm />
        </div>

        <div className="flex justify-end gap-2 border-t border-foreground/10 px-6 py-4">
          <Button variant="ghost" onClick={onClose}>
            {t('contactPrompt.cancel')}
          </Button>
          <Button onClick={onContinue}>{t('contactPrompt.continue')}</Button>
        </div>
      </div>
    </ModalShell>
  );
}
