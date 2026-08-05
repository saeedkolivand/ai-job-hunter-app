/**
 * Shared helpers for importing a resume from a profile URL.
 * Used by both `ResumeInputCard` (editor text) and `ProfileUrlImport`
 * (onboarding / settings — persists a document).
 */

/** Whether a profile URL is supported for import (currently LinkedIn only). */
export function isSupportedProfileUrl(url: string): boolean {
  return url.toLowerCase().includes('linkedin.com/in/');
}
