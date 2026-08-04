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
  /**
   * Rejects on failure (unmanaged store, invalid payload, storage error) —
   * there is no in-band `{ error }` shape to inspect. A Tauri command
   * returning `Result` rejects the invoke promise on `Err`, which is what
   * lets `useSaveContactProfile`'s `onError` fire; an in-band error field
   * that no caller reads is a silent, permanent save failure.
   */
  set(profile: ContactProfile): Promise<{ success: true }>;
  /**
   * The stored profile's header contact line as markdown, localized for `lang`
   * — built by the single shared `ContactProfile::header_markdown` (Rust), never
   * re-implemented here, so the ordering rules can't drift between the two
   * languages. `''` when the profile has nothing to contribute.
   */
  headerLine(lang: string): Promise<string>;
}

export const CONTACT_PROFILE_CHANNELS = {
  get: 'contact_profile_get',
  set: 'contact_profile_set',
  headerLine: 'contact_profile_header_line',
} as const;
