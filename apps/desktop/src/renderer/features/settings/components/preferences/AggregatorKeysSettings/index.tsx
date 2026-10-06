import { Search } from 'lucide-react';

import { PROVIDER_SLOTS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, SettingsSection } from '@ajh/ui';

import { useOpenExternal, useScrapingSettings } from '@/services';

import { AggregatorKeyField } from './AggregatorKeyField';
import { ApifyLinkedinSection } from './ApifyLinkedinSection';

const ADZUNA_DOCS_URL = 'https://developer.adzuna.com';
const JOOBLE_DOCS_URL = 'https://jooble.org/api/about';

// ── Public section component ──────────────────────────────────────────────────

export function AggregatorKeysSettings() {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();
  const { data: scrapingSettings } = useScrapingSettings();

  return (
    <SettingsSection icon={Search} label={t('settings.aggregatorKeys.title')}>
      <p className="mb-3 text-xs leading-relaxed text-foreground/50">
        {t('settings.aggregatorKeys.description')}{' '}
        <Button
          variant="unstyled"
          onClick={() => void openExternal.mutateAsync(ADZUNA_DOCS_URL)}
          className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
        >
          developer.adzuna.com
        </Button>{' '}
        {t('settings.aggregatorKeys.descriptionSuffix')}
      </p>

      <div className="space-y-4">
        <AggregatorKeyField
          slot={PROVIDER_SLOTS.adzunaAppId}
          labelKey="settings.aggregatorKeys.adzunaAppId.label"
          placeholderKey="settings.aggregatorKeys.adzunaAppId.placeholder"
          connectedKey="settings.aggregatorKeys.adzunaAppId.connected"
          removeConfirmTitleKey="settings.aggregatorKeys.adzunaAppId.removeConfirmTitle"
          removeConfirmDescKey="settings.aggregatorKeys.adzunaAppId.removeConfirmDesc"
        />

        <AggregatorKeyField
          slot={PROVIDER_SLOTS.adzunaAppKey}
          labelKey="settings.aggregatorKeys.adzunaAppKey.label"
          placeholderKey="settings.aggregatorKeys.adzunaAppKey.placeholder"
          connectedKey="settings.aggregatorKeys.adzunaAppKey.connected"
          removeConfirmTitleKey="settings.aggregatorKeys.adzunaAppKey.removeConfirmTitle"
          removeConfirmDescKey="settings.aggregatorKeys.adzunaAppKey.removeConfirmDesc"
        />

        <AggregatorKeyField
          slot={PROVIDER_SLOTS.jsearchKey}
          labelKey="settings.aggregatorKeys.jsearchKey.label"
          placeholderKey="settings.aggregatorKeys.jsearchKey.placeholder"
          connectedKey="settings.aggregatorKeys.jsearchKey.connected"
          removeConfirmTitleKey="settings.aggregatorKeys.jsearchKey.removeConfirmTitle"
          removeConfirmDescKey="settings.aggregatorKeys.jsearchKey.removeConfirmDesc"
        />

        {/* Jooble — last-resort fallback fired only once Adzuna + JSearch both
            come up empty/erroring (see aggregator/fallback.rs: primary_chain). */}
        <div className="space-y-1.5">
          <AggregatorKeyField
            slot={PROVIDER_SLOTS.joobleKey}
            labelKey="settings.aggregatorKeys.joobleKey.label"
            placeholderKey="settings.aggregatorKeys.joobleKey.placeholder"
            connectedKey="settings.aggregatorKeys.joobleKey.connected"
            removeConfirmTitleKey="settings.aggregatorKeys.joobleKey.removeConfirmTitle"
            removeConfirmDescKey="settings.aggregatorKeys.joobleKey.removeConfirmDesc"
          />
          <p className="text-xs text-foreground/40">
            {t('settings.aggregatorKeys.joobleKey.helper')}{' '}
            <Button
              variant="unstyled"
              onClick={() => void openExternal.mutateAsync(JOOBLE_DOCS_URL)}
              className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
            >
              jooble.org/api/about
            </Button>
          </p>
        </div>

        {/* Apify API token — credential slot for the LinkedIn (Apify) provider */}
        <AggregatorKeyField
          slot={PROVIDER_SLOTS.apifyToken}
          labelKey="settings.aggregatorKeys.apifyToken.label"
          placeholderKey="settings.aggregatorKeys.apifyToken.placeholder"
          connectedKey="settings.aggregatorKeys.apifyToken.connected"
          removeConfirmTitleKey="settings.aggregatorKeys.apifyToken.removeConfirmTitle"
          removeConfirmDescKey="settings.aggregatorKeys.apifyToken.removeConfirmDesc"
        />

        {/* Non-secret Apify LinkedIn settings (toggle + actor override + cost warning) */}
        {scrapingSettings && (
          <div className="border-t border-foreground/10 pt-2">
            <ApifyLinkedinSection settings={scrapingSettings} />
          </div>
        )}

        {/* Comeet board credentials — company UID + API token. No enable toggle:
            the board activates once both credentials are present. */}
        <div className="space-y-4 border-t border-foreground/10 pt-4">
          <AggregatorKeyField
            slot={PROVIDER_SLOTS.comeetCompanyUid}
            labelKey="settings.aggregatorKeys.comeetCompanyUid.label"
            placeholderKey="settings.aggregatorKeys.comeetCompanyUid.placeholder"
            connectedKey="settings.aggregatorKeys.comeetCompanyUid.connected"
            removeConfirmTitleKey="settings.aggregatorKeys.comeetCompanyUid.removeConfirmTitle"
            removeConfirmDescKey="settings.aggregatorKeys.comeetCompanyUid.removeConfirmDesc"
          />

          <AggregatorKeyField
            slot={PROVIDER_SLOTS.comeetApiToken}
            labelKey="settings.aggregatorKeys.comeetApiToken.label"
            placeholderKey="settings.aggregatorKeys.comeetApiToken.placeholder"
            connectedKey="settings.aggregatorKeys.comeetApiToken.connected"
            removeConfirmTitleKey="settings.aggregatorKeys.comeetApiToken.removeConfirmTitle"
            removeConfirmDescKey="settings.aggregatorKeys.comeetApiToken.removeConfirmDesc"
          />
        </div>
      </div>
    </SettingsSection>
  );
}
