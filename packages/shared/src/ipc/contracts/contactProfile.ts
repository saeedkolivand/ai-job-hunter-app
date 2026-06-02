/**
 * Contact profile — the single source of truth for the document header contact
 * line (name fields → clickable links), localized per language. Built from the
 * named fields here, never scavenged from the résumé's link pool, so a company
 * link can't displace the candidate's own profile / site.
 */

/** A free-text value with optional per-language (ISO-639-1) overrides. */
export interface LocalizedText {
  /** Value used when no language override matches. */
  default: string;
  /** ISO-639-1 (`de`, `en`, …) → localized value. */
  byLang?: Record<string, string>;
}

/** One additional labelled link beyond the named platform fields. */
export interface ContactLink {
  label: string;
  url: string;
}

/** Header contact fields, by name. Every field is optional. */
export interface ContactProfile {
  fullName?: string;
  email?: string;
  phone?: string;
  location?: LocalizedText;
  linkedin?: string;
  github?: string;
  website?: string;
  extraLinks?: ContactLink[];
  /**
   * Optional candidate photo as a `data:image/…;base64,…` URL produced by the
   * photo-upload control (decoded, square-cropped, downscaled, EXIF-stripped).
   * File paths are never accepted — local-only, never sent over the network.
   */
  photo?: string;
}

export interface ContactProfileContract {
  get(): Promise<ContactProfile>;
  set(profile: ContactProfile): Promise<{ success?: boolean; error?: string }>;
}

export const CONTACT_PROFILE_CHANNELS = {
  get: 'contact_profile_get',
  set: 'contact_profile_set',
} as const;
