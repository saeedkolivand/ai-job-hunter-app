import type { PrivacyResetResult } from '@ajh/shared';

/**
 * i18n key for a factory-reset outcome that needs the user's attention, or
 * `null` when the reset fully succeeded. A store that failed to clear takes
 * precedence over the board-login-session warning.
 */
export function resetAppWarningKey(res: PrivacyResetResult): string | null {
  if (res.success) return null;
  if (res.failedStores && res.failedStores.length > 0) return 'settings.privacy.resetAppIncomplete';
  return 'settings.privacy.resetAppPartial';
}
