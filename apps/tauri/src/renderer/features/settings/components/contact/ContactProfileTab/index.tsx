import { Contact } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { SettingsSection } from '@ajh/ui';

import { ContactProfileForm } from '@/components/contact/ContactProfileForm';
import { ApplicantDetailsSection } from '@/features/settings/components/contact/ApplicantDetailsSection';

/**
 * Edit the contact profile — the single source of truth for the document header
 * contact line. The résumé, cover letter, and DOCX all build their header from
 * these named fields (never the résumé's company-link pool), so a personal
 * LinkedIn / Website can't be displaced by an employer URL. Seeded from an
 * uploaded résumé on import, then freely editable here. The form itself is shared
 * with the first-run pre-generation prompt ([`ContactProfileForm`]).
 */
export function ContactProfileTab() {
  const { t } = useTranslation();

  return (
    <>
      <div data-settings-anchor="contact-profile">
        <SettingsSection icon={Contact} label={t('settings.contactProfile.title')}>
          <p className="mb-4 text-xs text-foreground/55">
            {t('settings.contactProfile.description')}
          </p>
          <ContactProfileForm />
        </SettingsSection>
      </div>
      <div data-settings-anchor="contact-applicant">
        <ApplicantDetailsSection />
      </div>
    </>
  );
}
