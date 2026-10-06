import { useTranslation } from '@ajh/translations';
import { Input } from '@ajh/ui';

import { FieldError, FieldLabel } from '../DetailChrome';

/**
 * Id of the single shared hint under the recipient pair. A literal (not `useId`)
 * because BOTH fields must point at the same node, and only one instance of this
 * tab is ever mounted.
 */
const CONTACT_HINT_ID = 'applyemail-contact-hint';

interface Props {
  name: string;
  email: string;
  nameError: boolean;
  emailError: string | null;
  onNameChange: (value: string) => void;
  onEmailChange: (value: string) => void;
  onNameBlur: () => void;
  onEmailBlur: (value: string) => void;
}

/** The recipient name/email pair (the application's canonical contact) + its shared hint. */
export function EmailRecipientFields({
  name,
  email,
  nameError,
  emailError,
  onNameChange,
  onEmailChange,
  onNameBlur,
  onEmailBlur,
}: Props) {
  const { t } = useTranslation();
  return (
    <>
      <div className="grid gap-3 @md:grid-cols-2">
        <div className="flex flex-col gap-1">
          <FieldLabel htmlFor="applyemail-recipient-name">
            {t('applications.detail.email.recipientNameLabel')}
          </FieldLabel>
          {/* ONE hint for the pair, rendered once below and referenced by BOTH
              fields — a screen-reader user hears the consequence while focused
              on either, and a sighted user reads it once, not twice. */}
          <Input
            id="applyemail-recipient-name"
            variant="default"
            placeholder={t('applications.detail.email.recipientNamePlaceholder')}
            value={name}
            onChange={(e) => onNameChange(e.target.value)}
            onBlur={onNameBlur}
            aria-describedby={CONTACT_HINT_ID}
          />
          {nameError && <FieldError>{t('applications.detail.contactSaveError')}</FieldError>}
        </div>
        <div className="flex flex-col gap-1">
          <FieldLabel htmlFor="applyemail-recipient-email">
            {t('applications.detail.email.recipientEmailLabel')}
          </FieldLabel>
          <Input
            id="applyemail-recipient-email"
            type="email"
            variant="default"
            placeholder={t('applications.detail.email.recipientEmailPlaceholder')}
            value={email}
            onChange={(e) => onEmailChange(e.target.value)}
            onBlur={(e) => onEmailBlur(e.target.value)}
            aria-describedby={CONTACT_HINT_ID}
          />
          {emailError && <FieldError>{emailError}</FieldError>}
        </div>
      </div>

      <p id={CONTACT_HINT_ID} className="text-fine-print text-foreground/70">
        {t('applications.detail.email.recipientHint')}
      </p>
    </>
  );
}
