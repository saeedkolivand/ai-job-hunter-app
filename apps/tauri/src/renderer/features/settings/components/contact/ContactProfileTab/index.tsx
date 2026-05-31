import { Contact } from 'lucide-react';
import { useEffect, useState } from 'react';

import type { ContactProfile } from '@ajh/shared';
import { Input, SettingsSection } from '@ajh/ui';

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
 * Follows the settings convention: changes persist on blur, no explicit save.
 */
export function ContactProfileTab() {
  const { data: profile } = useContactProfile();
  const { mutate: save } = useSaveContactProfile();

  const [fullName, setFullName] = useState('');
  const [email, setEmail] = useState('');
  const [phone, setPhone] = useState('');
  const [locationDefault, setLocationDefault] = useState('');
  const [locationDe, setLocationDe] = useState('');
  const [linkedin, setLinkedin] = useState('');
  const [github, setGithub] = useState('');
  const [website, setWebsite] = useState('');

  // Hydrate the form once the stored profile loads (or changes).
  useEffect(() => {
    if (!profile) return;
    setFullName(profile.fullName ?? '');
    setEmail(profile.email ?? '');
    setPhone(profile.phone ?? '');
    setLocationDefault(profile.location?.default ?? '');
    setLocationDe(profile.location?.byLang?.de ?? '');
    setLinkedin(profile.linkedin ?? '');
    setGithub(profile.github ?? '');
    setWebsite(profile.website ?? '');
  }, [profile]);

  // Persist the whole profile from current field state (called on blur).
  const persist = () => {
    const clean = (s: string) => s.trim() || undefined;
    const byLang: Record<string, string> = {};
    if (locationDe.trim()) byLang.de = locationDe.trim();

    const next: ContactProfile = {
      fullName: clean(fullName),
      email: clean(email),
      phone: clean(phone),
      location: locationDefault.trim()
        ? {
            default: locationDefault.trim(),
            byLang: Object.keys(byLang).length ? byLang : undefined,
          }
        : undefined,
      linkedin: clean(linkedin),
      github: clean(github),
      website: clean(website),
      // Preserve any extra links seeded on import — the form doesn't edit them.
      extraLinks: profile?.extraLinks,
    };
    save(next);
  };

  return (
    <SettingsSection icon={Contact} label="Contact Profile">
      <p className="mb-4 text-xs text-foreground/55">
        The header on your résumé and cover letter is built from these fields — your name, email,
        phone, location, and personal profile links. Editing them keeps every generated document
        correct and consistent. Changes save automatically.
      </p>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-name">
            Full name
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
            Email
          </label>
          <Input
            id="cp-email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            onBlur={persist}
            placeholder="name@example.com"
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-phone">
            Phone
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
            Location (English documents)
          </label>
          <Input
            id="cp-location"
            value={locationDefault}
            onChange={(e) => setLocationDefault(e.target.value)}
            onBlur={persist}
            placeholder="Netherlands"
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-location-de">
            Location (German documents)
          </label>
          <Input
            id="cp-location-de"
            value={locationDe}
            onChange={(e) => setLocationDe(e.target.value)}
            onBlur={persist}
            placeholder="Niederlande"
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-linkedin">
            LinkedIn
          </label>
          <Input
            id="cp-linkedin"
            value={linkedin}
            onChange={(e) => setLinkedin(e.target.value)}
            onBlur={persist}
            placeholder="https://www.linkedin.com/in/your-profile/"
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-github">
            GitHub
          </label>
          <Input
            id="cp-github"
            value={github}
            onChange={(e) => setGithub(e.target.value)}
            onBlur={persist}
            placeholder="https://github.com/your-handle"
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-website">
            Website
          </label>
          <Input
            id="cp-website"
            value={website}
            onChange={(e) => setWebsite(e.target.value)}
            onBlur={persist}
            placeholder="https://your-site.com"
          />
        </div>
      </div>
    </SettingsSection>
  );
}
