import { Check, Loader2 } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, Input, useNotification } from '@ajh/ui';

import { useSearchSettings, useSetSearchSettings } from '@/services';

/**
 * The SearXNG-compatible search server URL (SearXNG, RustXNG). No key, so it is a plain
 * URL field rather than a `SearchKeyField`. Saving an empty field clears it; the backend's
 * validation message is shown inline.
 */
export function SearxngField() {
  const { t } = useTranslation();
  const notify = useNotification();
  const { data } = useSearchSettings();
  const setSettings = useSetSearchSettings();
  const saved = data?.searxngUrl ?? '';
  const [draft, setDraft] = useState<string | null>(null);
  const value = draft ?? saved;
  const dirty = value.trim() !== saved;

  const handleSave = () => {
    if (!dirty || setSettings.isPending) return;
    const url = value.trim();
    const clearing = url === '';
    setSettings.mutate(
      { searxngUrl: clearing ? null : url },
      {
        onSuccess: () => {
          setDraft(null);
          notify.success({
            message: t(
              clearing
                ? 'settings.companyResearch.searxngRemoved'
                : 'settings.companyResearch.searxngSaved'
            ),
          });
        },
      }
    );
  };

  return (
    <div className="space-y-2">
      <p className="text-xs leading-relaxed text-foreground/50">
        {t('settings.companyResearch.searxngDescription')}
      </p>
      <div className="flex items-center gap-2">
        <Input
          type="url"
          value={value}
          onChange={(e) => {
            setDraft(e.target.value);
            if (setSettings.isError) setSettings.reset();
          }}
          onKeyDown={(e) => e.key === 'Enter' && handleSave()}
          placeholder={t('settings.companyResearch.searxngPlaceholder')}
          aria-label={t('settings.companyResearch.searxngLabel')}
          aria-invalid={setSettings.isError}
          className="w-full text-sm"
        />
        <Button
          variant="glass"
          disabled={!dirty || setSettings.isPending}
          onClick={handleSave}
          className={dirty && !setSettings.isPending ? 'ring-1 ring-brand/20' : ''}
        >
          {setSettings.isPending ? (
            <Loader2 size={13} className="animate-spin" />
          ) : (
            <>
              <Check size={12} /> {t('settings.companyResearch.save')}
            </>
          )}
        </Button>
      </div>
      {setSettings.isError && (
        <p role="alert" className="text-xs text-red-400">
          {setSettings.error.message}
        </p>
      )}
    </div>
  );
}
