import { Search } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { SettingsSection } from '@ajh/ui';

import { SearchKeyField } from './SearchKeyField';

const OLLAMA_KEYS_URL = 'https://ollama.com/settings/keys';
const EXA_KEYS_URL = 'https://dashboard.exa.ai/api-keys';

/**
 * Company-research settings — the two keys that can power the web search behind
 * a research brief.
 *
 * Research prefers the **active AI provider's own** search, so cloud and CLI
 * providers work with the key they already have. The two slots here cover the
 * providers that cannot: the Ollama family needs the free Ollama account key,
 * and anything with no search at all (every `openai-compatible` gateway, or a
 * keyless local Ollama) falls back to Exa when a key is stored.
 *
 * Fallback only — a provider that can already search is never redirected to Exa,
 * so adding a key here cannot silently move existing spend to another vendor.
 */
export function CompanyResearchSettings() {
  const { t } = useTranslation();

  return (
    <SettingsSection icon={Search} label={t('settings.companyResearch.title')}>
      <p className="mb-3 text-xs leading-relaxed text-foreground/50">
        {t('settings.companyResearch.description')}
      </p>

      <SearchKeyField
        provider="ollama-cloud"
        keysUrl={OLLAMA_KEYS_URL}
        connectedLabel={t('settings.companyResearch.connected')}
        getKeyLabel={t('settings.companyResearch.getKeyAt')}
        placeholder={t('settings.companyResearch.keyPlaceholder')}
        note={t('settings.companyResearch.sameKeyNote')}
        fieldLabel={t('settings.companyResearch.ollamaFieldLabel')}
        savedMessage={t('settings.companyResearch.saved')}
        removedMessage={t('settings.companyResearch.removed')}
        removeConfirmTitle={t('settings.companyResearch.removeConfirmTitle')}
        removeConfirmDescription={t('settings.companyResearch.removeConfirmDesc')}
      />

      <div className="mt-4 border-t border-[var(--border-clear)] pt-3">
        <p className="mb-2 text-xs leading-relaxed text-foreground/50">
          {t('settings.companyResearch.fallbackDescription')}
        </p>
        <SearchKeyField
          provider="exa"
          keysUrl={EXA_KEYS_URL}
          connectedLabel={t('settings.companyResearch.exaConnected')}
          getKeyLabel={t('settings.companyResearch.exaGetKeyAt')}
          placeholder={t('settings.companyResearch.exaKeyPlaceholder')}
          note={t('settings.companyResearch.exaNote')}
          fieldLabel={t('settings.companyResearch.exaFieldLabel')}
          savedMessage={t('settings.companyResearch.exaSaved')}
          removedMessage={t('settings.companyResearch.exaRemoved')}
          removeConfirmTitle={t('settings.companyResearch.exaRemoveConfirmTitle')}
          removeConfirmDescription={t('settings.companyResearch.exaRemoveConfirmDesc')}
        />
      </div>
    </SettingsSection>
  );
}
