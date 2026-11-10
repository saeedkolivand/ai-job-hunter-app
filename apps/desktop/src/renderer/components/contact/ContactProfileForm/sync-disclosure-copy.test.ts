import { describe, expect, it } from 'vitest';

import de from '../../../../../../../packages/translations/src/locales/de/translation.json';
import en from '../../../../../../../packages/translations/src/locales/en/translation.json';

// #1438: the contact photo and referrals are in the opt-in self-hosted sync
// payload (#1402), so their copy must never promise they stay on-device.
describe('sync-covered data copy', () => {
  it.each([
    ['en photo', en.settings.contactProfile.photoHint],
    ['en referral', en.autopilot.referral.privacy],
    ['de photo', de.settings.contactProfile.photoHint],
    ['de referral', de.autopilot.referral.privacy],
  ])('%s does not claim device-only storage', (_, copy) => {
    expect(copy).not.toMatch(
      /never uploaded|device only|niemals hochgeladen|nur (lokal )?auf Ihrem Gerät/i
    );
    expect(copy).toMatch(/sync/i);
  });

  it.each([en.settings.contactProfile.photoHint, de.settings.contactProfile.photoHint])(
    'photo hint discloses backups: %s',
    (copy) => expect(copy).toMatch(/backup|Sicherung/i)
  );
});
