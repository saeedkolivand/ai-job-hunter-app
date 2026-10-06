import { z } from 'zod';

import type { ContactProfile } from '@ajh/shared';

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
export const contactSchema = z.object({
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

export type ContactFormValues = z.infer<typeof contactSchema>;

export const EMPTY_VALUES: ContactFormValues = {
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
export function toFormValues(profile: ContactProfile): ContactFormValues {
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
