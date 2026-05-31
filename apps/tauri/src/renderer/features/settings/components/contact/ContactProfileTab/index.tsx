import { Contact } from 'lucide-react';
import { useEffect, useState } from 'react';

import type { ContactProfile } from '@ajh/shared';
import { Input, SettingsSection } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useContactProfile, useSaveContactProfile } from '@/services';

const FIELD_CLASS = 'flex flex-col gap-1.5';
const LABEL_CLASS = 'text-xs font-medium text-foreground/70';

/**
 * Edit the contact profile — the single source of truth for the document header
 * contact line. The résumé, cover letter, and DOCX all build their header from
 * these named fields (never the résumé's company-link pool), so a personal
 * LinkedIn / Website can't be displaced by an employer URL. Seeded from an
 * uploaded résumé on import, then freely editable here.
 *
 * Location is a single value used for every document language: the backend's
 * `LocalizedText` still supports per-language overrides, but we no longer expose
 * an input per language (that doesn't scale as languages are added), so the form
 * writes only `default` and clears any legacy override on save.
 *
 * Follows the settings convention: changes persist on blur, no explicit save.
 */
export function ContactProfileTab() {
  const { t } = useTranslation();
  const { data: profile } = useContactProfile();
  const { mutate: save } = useSaveContactProfile();

  const [fullName, setFullName] = useState('');
  const [email, setEmail] = useState('');
  const [phone, setPhone] = useState('');
  const [location, setLocation] = useState('');
  const [linkedin, setLinkedin] = useState('');
  const [github, setGithub] = useState('');
  const [website, setWebsite] = useState('');

  // Hydrate the form once the stored profile loads (or changes).
  useEffect(() => {
    if (!profile) return;
    setFullName(profile.fullName ?? '');
    setEmail(profile.email ?? '');
    setPhone(profile.phone ?? '');
    setLocation(profile.location?.default ?? '');
    setLinkedin(profile.linkedin ?? '');
    setGithub(profile.github ?? '');
    setWebsite(profile.website ?? '');
  }, [profile]);

  // Persist the whole profile from current field state (called on blur).
  const persist = () => {
    const clean = (s: string) => s.trim() || undefined;

    const next: ContactProfile = {
      fullName: clean(fullName),
      email: clean(email),
      phone: clean(phone),
      // One location for every document language — no per-language overrides.
      location: location.trim() ? { default: location.trim() } : undefined,
      linkedin: clean(linkedin),
      github: clean(github),
      website: clean(website),
      // Preserve any extra links seeded on import — the form doesn't edit them.
      extraLinks: profile?.extraLinks,
    };
    save(next);
  };

  return (
    <SettingsSection icon={Contact} label={t('settings.contactProfile.title')}>
      <p className="mb-4 text-xs text-foreground/55">{t('settings.contactProfile.description')}</p>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-name">
            {t('settings.contactProfile.fullName')}
          </label>
          <Input
            id="cp-name"
            value={fullName}
            onChange={(e) => setFullName(e.target.value)}
            onBlur={persist}
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-email">
            {t('settings.contactProfile.email')}
          </label>
          <Input
            id="cp-email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            onBlur={persist}
            placeholder={t('settings.contactProfile.emailPlaceholder')}
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-phone">
            {t('settings.contactProfile.phone')}
          </label>
          <Input
            id="cp-phone"
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            onBlur={persist}
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-location">
            {t('settings.contactProfile.location')}
          </label>
          <Input
            id="cp-location"
            value={location}
            onChange={(e) => setLocation(e.target.value)}
            onBlur={persist}
            placeholder={t('settings.contactProfile.locationPlaceholder')}
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-linkedin">
            {t('settings.contactProfile.linkedin')}
          </label>
          <Input
            id="cp-linkedin"
            value={linkedin}
            onChange={(e) => setLinkedin(e.target.value)}
            onBlur={persist}
            placeholder={t('settings.contactProfile.linkedinPlaceholder')}
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-github">
            {t('settings.contactProfile.github')}
          </label>
          <Input
            id="cp-github"
            value={github}
            onChange={(e) => setGithub(e.target.value)}
            onBlur={persist}
            placeholder={t('settings.contactProfile.githubPlaceholder')}
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-website">
            {t('settings.contactProfile.website')}
          </label>
          <Input
            id="cp-website"
            value={website}
            onChange={(e) => setWebsite(e.target.value)}
            onBlur={persist}
            placeholder={t('settings.contactProfile.websitePlaceholder')}
          />
        </div>
      </div>
    </SettingsSection>
  );
}
