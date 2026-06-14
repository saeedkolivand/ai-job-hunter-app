import { Camera, Plus, Trash2, UserRound } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Controller, useFieldArray, useForm } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';

import type { ContactProfile } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Input, LocationInput } from '@ajh/ui';

import { PhotoProcessingError, processPhotoFile } from '@/lib/photo';
import { useAppClient } from '@/providers/AppClientProvider';
import { useContactProfile, useSaveContactProfile } from '@/services';

const FIELD_CLASS = 'flex flex-col gap-1.5';
const LABEL_CLASS = 'text-xs font-medium text-foreground/70';

const isBlank = (v: string | undefined): boolean => !v || !v.trim();

/** Accepts http(s) URLs only; non-pedantic. Blank passes. */
function isValidUrl(value: string): boolean {
  if (isBlank(value)) return true;
  try {
    const url = new URL(value.trim());
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
}

/** A blank string or a valid http(s) URL. Messages are i18n keys. */
const urlField = z.string().refine(isValidUrl, { message: 'settings.contactProfile.urlInvalid' });

/**
 * Light, NON-blocking schema for the contact form. The form auto-saves on blur
 * (no submit), so these refinements only surface inline hints — they never gate
 * persistence. Every field is a plain string; empty strings are the "unset"
 * value and are treated as blank.
 */
const contactSchema = z.object({
  fullName: z.string(),
  email: z.string().refine((v) => isBlank(v) || z.string().email().safeParse(v.trim()).success, {
    message: 'settings.contactProfile.emailInvalid',
  }),
  phone: z.string(),
  location: z.string(),
  linkedin: urlField,
  github: urlField,
  website: urlField,
  extraLinks: z.array(z.object({ label: z.string(), url: urlField })),
});

type ContactFormValues = z.infer<typeof contactSchema>;

const EMPTY_VALUES: ContactFormValues = {
  fullName: '',
  email: '',
  phone: '',
  location: '',
  linkedin: '',
  github: '',
  website: '',
  extraLinks: [],
};

/** Map a stored profile into the flat form value shape. */
function toFormValues(profile: ContactProfile): ContactFormValues {
  return {
    fullName: profile.fullName ?? '',
    email: profile.email ?? '',
    phone: profile.phone ?? '',
    location: profile.location?.default ?? '',
    linkedin: profile.linkedin ?? '',
    github: profile.github ?? '',
    website: profile.website ?? '',
    extraLinks: (profile.extraLinks ?? []).map((l) => ({ label: l.label, url: l.url })),
  };
}

/**
 * The editable contact-profile form — the single source of truth for the document
 * header contact line (name fields → clickable links). Shared between the Settings
 * tab and the first-run pre-generation prompt, so both edit the exact same fields.
 *
 * Uses its OWN isolated react-hook-form instance (never the surrounding builder's
 * `FormProvider`): it is rendered inside the Résumé Builder's form in StepContact,
 * yet it persists to the contact profile — a separate store from the builder form —
 * so it must not share that context. The saved profile is the source of truth, so
 * `reset()`-on-load is correct here (unlike the builder's draft-preserving form).
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

  const {
    control,
    reset,
    getValues,
    formState: { errors },
  } = useForm<ContactFormValues>({
    defaultValues: EMPTY_VALUES,
    resolver: zodResolver(contactSchema),
    // Auto-save form: validate on blur for inline hints, never to gate persist.
    mode: 'onBlur',
  });

  const { fields, append, remove } = useFieldArray({ control, name: 'extraLinks' });

  // The candidate photo as a bounded `data:` URL (used by the photo templates).
  // Not an RHF field — it isn't a text input and persists immediately on its own.
  const [photo, setPhoto] = useState('');
  const [photoError, setPhotoError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  // Hydrate the form when the stored profile's IDENTITY changes. `useContactProfile`
  // re-emits a fresh object reference on background/window-focus refetches even when
  // the data is identical — resetting on every emit would clobber unsaved in-progress
  // edits. Track the last-applied signature and reset only when the content differs.
  const lastAppliedRef = useRef<string | null>(null);
  useEffect(() => {
    if (!profile) return;
    const signature = JSON.stringify(profile);
    if (lastAppliedRef.current === signature) return;
    lastAppliedRef.current = signature;
    reset(toFormValues(profile));
    setPhoto(profile.photo ?? '');
  }, [profile, reset]);

  // Build the whole profile from the current form values. Extra links keep only
  // rows with a URL (an empty draft row is dropped) and are trimmed. `photo` is
  // read from local state. Callers may override individual keys for async/just-
  // mutated values that RHF hasn't flushed yet — `extraLinks` is honored here
  // (it's the field array, not a plain key) so a removal persists the next set.
  const buildProfile = (
    overrides: Partial<ContactProfile> & { extraLinks?: { label: string; url: string }[] } = {}
  ): ContactProfile => {
    const v = getValues();
    const clean = (s: string) => s.trim() || undefined;
    const { extraLinks: extraOverride, ...rest } = overrides;
    const source = extraOverride ?? v.extraLinks;
    const extra = source
      .map((l) => ({ label: l.label.trim(), url: l.url.trim() }))
      .filter((l) => l.url !== '');
    return {
      fullName: clean(v.fullName),
      email: clean(v.email),
      phone: clean(v.phone),
      // One location for every document language — no per-language overrides.
      location: v.location.trim() ? { default: v.location.trim() } : undefined,
      linkedin: clean(v.linkedin),
      github: clean(v.github),
      website: clean(v.website),
      extraLinks: extra.length ? extra : undefined,
      photo: clean(photo),
      ...rest,
    };
  };

  // Persist on blur (named fields + extra-link edits).
  const persist = () => save(buildProfile());

  // Location commits on select/clear (LocationInput has no blur), so persist
  // immediately with the next value rather than the stale form one.
  const persistLocation = (next: string) =>
    save(buildProfile({ location: next.trim() ? { default: next.trim() } : undefined }));

  // Photo set/remove persist immediately with the next value (state is async),
  // so the data URL doesn't lag a render behind the saved profile.
  const persistPhoto = (next: string) => save(buildProfile({ photo: next.trim() || undefined }));

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

  // Add/remove persist immediately (mirror the prior behavior).
  const addLink = () => append({ label: '', url: '' });

  // RHF flushes the field-array removal AFTER this tick, so `getValues` here
  // still includes the removed row. Compute the next set up front and pass it as
  // an override so `buildProfile` persists the correct final list, not the stale one.
  const removeLink = (index: number) => {
    const next = getValues('extraLinks').filter((_, i) => i !== index);
    remove(index);
    save(buildProfile({ extraLinks: next }));
  };

  return (
    <>
      <div className="mb-6 flex items-center gap-4">
        {/* #20 — the avatar itself is the upload target: click to add or change
            the photo, with a camera overlay on hover. */}
        <Button
          variant="unstyled"
          type="button"
          onClick={() => fileRef.current?.click()}
          aria-label={t('settings.contactProfile.photoUpload')}
          className="group relative size-20 shrink-0 overflow-hidden rounded-full border border-white/10 bg-white/[0.03]"
        >
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
          <span className="absolute inset-0 flex items-center justify-center bg-black/45 text-white opacity-0 transition-opacity group-hover:opacity-100">
            <Camera className="size-5" />
          </span>
        </Button>
        <div className="flex flex-col gap-1.5">
          <span className={LABEL_CLASS}>{t('settings.contactProfile.photo')}</span>
          <p className="text-xs text-foreground/55">{t('settings.contactProfile.photoHint')}</p>
          <input
            ref={fileRef}
            type="file"
            accept="image/png,image/jpeg,image/webp,image/gif"
            className="hidden"
            onChange={onPickPhoto}
          />
          {photo && (
            <Button variant="ghost" onClick={removePhoto} className="gap-1.5 self-start">
              <Trash2 className="size-4" />
              {t('settings.contactProfile.photoRemove')}
            </Button>
          )}
          {photoError && <p className="text-xs text-amber-400/80">{photoError}</p>}
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-name">
            {t('settings.contactProfile.fullName')}
          </label>
          <Controller
            control={control}
            name="fullName"
            render={({ field }) => (
              <Input
                id="cp-name"
                value={field.value}
                onChange={field.onChange}
                onBlur={() => {
                  field.onBlur();
                  persist();
                }}
              />
            )}
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-email">
            {t('settings.contactProfile.email')}
          </label>
          <Controller
            control={control}
            name="email"
            render={({ field }) => (
              <Input
                id="cp-email"
                type="email"
                value={field.value}
                onChange={field.onChange}
                onBlur={() => {
                  field.onBlur();
                  persist();
                }}
                placeholder={t('settings.contactProfile.emailPlaceholder')}
                aria-invalid={errors.email ? true : undefined}
              />
            )}
          />
          {errors.email && (
            <p className="text-xs text-amber-400/80">{t(errors.email.message ?? '')}</p>
          )}
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-phone">
            {t('settings.contactProfile.phone')}
          </label>
          <Controller
            control={control}
            name="phone"
            render={({ field }) => (
              <Input
                id="cp-phone"
                value={field.value}
                onChange={field.onChange}
                onBlur={() => {
                  field.onBlur();
                  persist();
                }}
              />
            )}
          />
        </div>

        <div className={FIELD_CLASS}>
          <span className={LABEL_CLASS}>{t('settings.contactProfile.location')}</span>
          <Controller
            control={control}
            name="location"
            render={({ field }) => (
              <LocationInput
                value={field.value}
                onChange={(v) => {
                  field.onChange(v);
                  persistLocation(v);
                }}
                placeholder={t('settings.contactProfile.locationPlaceholder')}
                onFetchSuggestions={(q) => api.geocode.suggest(q)}
              />
            )}
          />
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-linkedin">
            {t('settings.contactProfile.linkedin')}
          </label>
          <Controller
            control={control}
            name="linkedin"
            render={({ field }) => (
              <Input
                id="cp-linkedin"
                value={field.value}
                onChange={field.onChange}
                onBlur={() => {
                  field.onBlur();
                  persist();
                }}
                placeholder={t('settings.contactProfile.linkedinPlaceholder')}
                aria-invalid={errors.linkedin ? true : undefined}
              />
            )}
          />
          {errors.linkedin && (
            <p className="text-xs text-amber-400/80">{t(errors.linkedin.message ?? '')}</p>
          )}
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-github">
            {t('settings.contactProfile.github')}
          </label>
          <Controller
            control={control}
            name="github"
            render={({ field }) => (
              <Input
                id="cp-github"
                value={field.value}
                onChange={field.onChange}
                onBlur={() => {
                  field.onBlur();
                  persist();
                }}
                placeholder={t('settings.contactProfile.githubPlaceholder')}
                aria-invalid={errors.github ? true : undefined}
              />
            )}
          />
          {errors.github && (
            <p className="text-xs text-amber-400/80">{t(errors.github.message ?? '')}</p>
          )}
        </div>

        <div className={FIELD_CLASS}>
          <label className={LABEL_CLASS} htmlFor="cp-website">
            {t('settings.contactProfile.website')}
          </label>
          <Controller
            control={control}
            name="website"
            render={({ field }) => (
              <Input
                id="cp-website"
                value={field.value}
                onChange={field.onChange}
                onBlur={() => {
                  field.onBlur();
                  persist();
                }}
                placeholder={t('settings.contactProfile.websitePlaceholder')}
                aria-invalid={errors.website ? true : undefined}
              />
            )}
          />
          {errors.website && (
            <p className="text-xs text-amber-400/80">{t(errors.website.message ?? '')}</p>
          )}
        </div>
      </div>

      <div className="mt-6 flex flex-col gap-2">
        <span className={LABEL_CLASS}>{t('settings.contactProfile.extraLinks')}</span>
        <p className="text-xs text-foreground/55">{t('settings.contactProfile.extraLinksHint')}</p>

        {fields.map((row, index) => (
          <div key={row.id} className="flex items-start gap-2">
            <Controller
              control={control}
              name={`extraLinks.${index}.label`}
              render={({ field }) => (
                <Input
                  aria-label={t('settings.contactProfile.linkLabel')}
                  value={field.value}
                  onChange={field.onChange}
                  onBlur={() => {
                    field.onBlur();
                    persist();
                  }}
                  placeholder={t('settings.contactProfile.linkLabelPlaceholder')}
                  className="w-1/3"
                />
              )}
            />
            <Controller
              control={control}
              name={`extraLinks.${index}.url`}
              render={({ field }) => (
                <Input
                  aria-label={t('settings.contactProfile.linkUrl')}
                  value={field.value}
                  onChange={field.onChange}
                  onBlur={() => {
                    field.onBlur();
                    persist();
                  }}
                  placeholder={t('settings.contactProfile.linkUrlPlaceholder')}
                  className="flex-1"
                  aria-invalid={errors.extraLinks?.[index]?.url ? true : undefined}
                />
              )}
            />
            <Button
              variant="ghost"
              className="h-8 w-8 p-0"
              aria-label={t('settings.contactProfile.removeLink')}
              onClick={() => removeLink(index)}
            >
              <Trash2 className="size-4" />
            </Button>
          </div>
        ))}

        <Button variant="ghost" onClick={addLink} className="mt-1 self-start gap-1.5">
          <Plus className="size-4" />
          {t('settings.contactProfile.addLink')}
        </Button>
      </div>
    </>
  );
}
