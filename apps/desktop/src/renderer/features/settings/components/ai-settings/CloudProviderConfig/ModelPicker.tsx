import { Inbox, Loader2 } from 'lucide-react';

import type { ProviderModelInfo } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Dropdown, EmptyState, ErrorState } from '@ajh/ui';

import { sortModelsNewestFirst } from '@/lib/ai-providers/model-sort';

interface ModelPickerProps {
  expandedModels: ProviderModelInfo[];
  providerModel: string;
  loading: boolean;
  /** `expandedModels` came from the last-good local cache (live fetch failed). */
  cached: boolean;
  error?: string;
  onSelectModel: (model: string) => void;
  onRecheck?: () => void;
}

/**
 * Model selector — four distinct states beyond the normal dropdown:
 * (1) still loading with nothing to show yet → a labelled spinner (not a bare
 * empty dropdown, which reads as "zero models"), (2) live fetch failed with no
 * cache and no stored selection to fall back to → the real failure, (3) fetch
 * succeeded but the catalogue is genuinely empty → a neutral empty state,
 * (4) options.length > 0 (fresh, cached, or a preserved unlisted selection) →
 * the dropdown, with a small note when the list is cached. Loading is checked
 * FIRST — it's the only state where none of the other guards fire.
 */
export function ModelPicker({
  expandedModels,
  providerModel,
  loading,
  cached,
  error,
  onSelectModel,
  onRecheck,
}: ModelPickerProps) {
  const { t } = useTranslation();
  const modelOptions = sortModelsNewestFirst(expandedModels).map((m) => ({
    value: m.name,
    label: m.displayName ?? m.name,
  }));

  // Keep a stored selection that fell out of the live/cached list selectable
  // — otherwise the trigger falls back to the placeholder and reads as a
  // reset config, when it's really just an unlisted model id (self-heals
  // once the live list includes it again).
  const options =
    providerModel && !modelOptions.some((o) => o.value === providerModel)
      ? [...modelOptions, { value: providerModel, label: providerModel }]
      : modelOptions;

  return (
    <div className="space-y-1.5">
      <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
        {t('settings.aiModel.title')}
      </div>
      {options.length === 0 && loading ? (
        <div
          role="status"
          aria-live="polite"
          className="flex items-center gap-2 text-xs text-foreground/40"
        >
          <Loader2 size={13} className="animate-spin" />
          {t('settings.aiModel.loading')}
        </div>
      ) : options.length === 0 && error ? (
        <ErrorState
          title={t('settings.aiModel.fetchFailedTitle')}
          description={error}
          onRetry={onRecheck}
          className="py-6"
        />
      ) : options.length === 0 ? (
        <EmptyState
          icon={Inbox}
          title={t('settings.aiModel.emptyTitle')}
          description={t('settings.aiModel.emptyDescription')}
          className="py-6"
        />
      ) : (
        <>
          <Dropdown
            options={options}
            value={providerModel}
            onChange={onSelectModel}
            placeholder="Select a model…"
          />
          {cached && (
            <p role="status" aria-live="polite" className="text-[10px] text-foreground/40">
              {t('settings.aiModel.cachedNotice')}
            </p>
          )}
        </>
      )}
    </div>
  );
}
