/**
 * Shared helpers for importing a resume from a profile URL.
 * Used by both `ResumeInputCard` (editor text) and `ProfileUrlImport`
 * (onboarding / settings — persists a document).
 */

/** Whether a profile URL is supported for import (currently LinkedIn only). */
export function isSupportedProfileUrl(url: string): boolean {
  return url.toLowerCase().includes('linkedin.com/in/');
}

/**
 * Whether a profile-import backend error is an authentication wall (the page
 * needs a logged-in LinkedIn session). Lets the UI show an actionable
 * "connect your account" hint instead of the raw scraper message.
 */
export function isProfileAuthError(error: string): boolean {
  const lower = error.toLowerCase();
  return (
    lower.includes('login') ||
    lower.includes('log in') ||
    lower.includes('sign in') ||
    lower.includes('not authenticated') ||
    lower.includes('unauthorized')
  );
}
