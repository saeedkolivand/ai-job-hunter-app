import { Plus, Trash2, Upload, UserRound } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import type { ContactProfile } from '@ajh/shared';
import { Button, Input, LocationInput } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { PhotoProcessingError, processPhotoFile } from '@/lib/photo';
import { useAppClient } from '@/providers/AppClientProvider';
import { useContactProfile, useSaveContactProfile } from '@/services';

const FIELD_CLASS = 'flex flex-col gap-1.5';
const LABEL_CLASS = 'text-xs font-medium text-foreground/70';

/** A locally-keyed extra-link row, so React keeps input focus across edits. */
interface LinkRow {
  id: number;
  label: string;
  url: string;
}

/**
 * The editable contact-profile form — the single source of truth for the document
 * header contact line (name fields → clickable links). Shared between the Settings
 * tab and the first-run pre-generation prompt, so both edit the exact same fields.
 *
 * "Other links" are the free-form extras beyond the named platforms (Dribbble,
 * Behance, a portfolio); they are autofilled from an imported résumé and editable
 * here as label/URL pairs.
 *
 * Location is a single value used for every document language (the backend's
 * `LocalizedText` still supports per-language overrides, but the form writes only
 * `default`). Changes persist on blur — no explicit save button.
 */
export function ContactProfileForm() {
  const { t } = useTranslation();
  const api = useAppClient();
  const { data: profile } = useContactProfile();
  const { mutate: save } = useSaveContactProfile();

  const [fullName, setFullName] = useState('');
  const [email, setEmail] = useState('');
  const [phone, setPhone] = useState('');
  const [location, setLocation] = useState('');
  const [linkedin, setLinkedin] = useState('');
  const [github, setGithub] = useState('');
  const [website, setWebsite] = useState('');
  const [extraLinks, setExtraLinks] = useState<LinkRow[]>([]);
  // The candidate photo as a bounded `data:` URL (used by the photo templates).
  const [photo, setPhoto] = useState('');
  const [photoError, setPhotoError] = useState<string | null>(null);
  const rowId = useRef(0);
  const fileRef = useRef<HTMLInputElement>(null);

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
    setPhoto(profile.photo ?? '');
    setExtraLinks(
      (profile.extraLinks ?? []).map((l) => ({ id: rowId.current++, label: l.label, url: l.url }))
    );
  }, [profile]);

  // Build the whole profile from current field state. Extra links keep only rows
  // with a URL (an empty draft row is dropped) and are trimmed.
  const buildProfile = (rows: LinkRow[]): ContactProfile => {
    const clean = (s: string) => s.trim() || undefined;
    const extra = rows
      .map((l) => ({ label: l.label.trim(), url: l.url.trim() }))
      .filter((l) => l.url !== '');
    return {
      fullName: clean(fullName),
      email: clean(email),
      phone: clean(phone),
      // One location for every document language — no per-language overrides.
      location: location.trim() ? { default: location.trim() } : undefined,
      linkedin: clean(linkedin),
      github: clean(github),
      website: clean(website),
      extraLinks: extra.length ? extra : undefined,
      photo: clean(photo),
    };
  };

  // Persist on blur (named fields + extra-link edits).
  const persist = () => save(buildProfile(extraLinks));

  // Location commits on select/clear (LocationInput has no blur), so persist
  // immediately with the next value rather than the stale closure one.
  const persistLocation = (next: string) =>
    save({
      ...buildProfile(extraLinks),
      location: next.trim() ? { default: next.trim() } : undefined,
    });

  // Photo set/remove persist immediately with the next value (state is async),
  // so the data URL doesn't lag a render behind the saved profile.
  const persistPhoto = (next: string) =>
    save({ ...buildProfile(extraLinks), photo: next.trim() || undefined });

  const onPickPhoto = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = ''; // allow re-picking the same file
    if (!file) return;
    try {
      const dataUrl = await processPhotoFile(file);
      setPhoto(dataUrl);
      setPhotoError(null);
      persistPhoto(dataUrl);
    } catch (err) {
      const kind = err instanceof PhotoProcessingError ? err.kind : 'decode';
      setPhotoError(
        t(
          kind === 'type'
            ? 'settings.contactProfile.photoErrorType'
            : kind === 'size'
              ? 'settings.contactProfile.photoErrorSize'
              : 'settings.contactProfile.photoErrorDecode'
        )
      );
    }
  };

  const removePhoto = () => {
    setPhoto('');
    setPhotoError(null);
    persistPhoto('');
  };

  const updateLink = (id: number, patch: Partial<LinkRow>) =>
    setExtraLinks((prev) => prev.map((l) => (l.id === id ? { ...l, ...patch } : l)));

  const addLink = () =>
    setExtraLinks((prev) => [...prev, { id: rowId.current++, label: '', url: '' }]);

  // Add/remove persist immediately with the next array (state is async).
  const removeLink = (id: number) => {
    const next = extraLinks.filter((l) => l.id !== id);
    setExtraLinks(next);
    save(buildProfile(next));
  };

  return (
    <>
      <div className="mb-6 flex items-center gap-4">
        <div className="size-20 shrink-0 overflow-hidden rounded-full border border-white/10 bg-white/[0.03]">
          {photo ? (
            <img
              src={photo}
              alt={t('settings.contactProfile.photoAlt')}
              className="size-full object-cover"
            />
          ) : (
            <div className="flex size-full items-center justify-center text-foreground/30">
              <UserRound className="size-8" />
            </div>
          )}
        </div>
        <div className="flex flex-col gap-1.5">
          <span className={LABEL_CLASS}>{t('settings.contactProfile.photo')}</span>
          <p className="text-xs text-foreground/55">{t('settings.contactProfile.photoHint')}</p>
          <div className="flex items-center gap-2">
            <input
              ref={fileRef}
              type="file"
              accept="image/png,image/jpeg,image/webp,image/gif"
              className="hidden"
              onChange={onPickPhoto}
            />
            <Button
              variant="ghost"
              size="sm"
              onClick={() => fileRef.current?.click()}
              className="gap-1.5"
            >
              <Upload className="size-4" />
              {t('settings.contactProfile.photoUpload')}
            </Button>
            {photo && (
              <Button variant="ghost" size="sm" onClick={removePhoto} className="gap-1.5">
                <Trash2 className="size-4" />
                {t('settings.contactProfile.photoRemove')}
              </Button>
            )}
          </div>
          {photoError && <p className="text-xs text-amber-400/80">{photoError}</p>}
        </div>
      </div>

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
          <span className={LABEL_CLASS}>{t('settings.contactProfile.location')}</span>
          <LocationInput
            value={location}
            onChange={(v) => {
              setLocation(v);
              persistLocation(v);
            }}
            placeholder={t('settings.contactProfile.locationPlaceholder')}
            onFetchSuggestions={(q) => api.geocode.suggest(q)}
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

      <div className="mt-6 flex flex-col gap-2">
        <span className={LABEL_CLASS}>{t('settings.contactProfile.extraLinks')}</span>
        <p className="text-xs text-foreground/55">{t('settings.contactProfile.extraLinksHint')}</p>

        {extraLinks.map((row) => (
          <div key={row.id} className="flex items-start gap-2">
            <Input
              aria-label={t('settings.contactProfile.linkLabel')}
              value={row.label}
              onChange={(e) => updateLink(row.id, { label: e.target.value })}
              onBlur={persist}
              placeholder={t('settings.contactProfile.linkLabelPlaceholder')}
              className="w-1/3"
            />
            <Input
              aria-label={t('settings.contactProfile.linkUrl')}
              value={row.url}
              onChange={(e) => updateLink(row.id, { url: e.target.value })}
              onBlur={persist}
              placeholder={t('settings.contactProfile.linkUrlPlaceholder')}
              className="flex-1"
            />
            <Button
              variant="ghost"
              size="sm"
              aria-label={t('settings.contactProfile.removeLink')}
              onClick={() => removeLink(row.id)}
            >
              <Trash2 className="size-4" />
            </Button>
          </div>
        ))}

        <Button variant="ghost" size="sm" onClick={addLink} className="mt-1 self-start gap-1.5">
          <Plus className="size-4" />
          {t('settings.contactProfile.addLink')}
        </Button>
      </div>
    </>
  );
}
