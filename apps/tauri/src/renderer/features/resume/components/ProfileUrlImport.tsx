import { Loader2 } from 'lucide-react';
import { useState } from 'react';

import { Button, Input, useNotification } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useImportDocument, useProfileImport } from '@/services';

interface Props {
  onImported?: (name: string) => void;
}

export function ProfileUrlImport({ onImported }: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const [url, setUrl] = useState('');

  const profileImport = useProfileImport();
  const importDocument = useImportDocument();

  const hasUrl = url.trim().length > 0;
  const canImport = hasUrl && !profileImport.isPending && !importDocument.isPending;
  const loading = profileImport.isPending || importDocument.isPending;

  const handleImport = async () => {
    if (!url.trim()) return;
    try {
      const result = await profileImport.mutateAsync(url.trim());

      if ('error' in result) {
        notify(result.error, 'error');
        return;
      }

      const encoder = new TextEncoder();
      const bytes = encoder.encode(result.text);
      const name = result.name
        ? `${result.name} (${result.platform}).txt`
        : `${result.platform} Profile.txt`;

      await importDocument.mutateAsync({ name, bytes: new Uint8Array(bytes), title: name });
      setUrl('');
      notify(t('resume.profileImport.success', { name: result.name ?? 'Profile' }), 'success');
      onImported?.(name);
    } catch (err) {
      notify(err instanceof Error ? err.message : t('resume.profileImport.failed'), 'error');
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-foreground/50">{t('resume.profileImport.description')}</p>
      <div className="flex gap-2">
        <Input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder={t('resumeInput.profileUrlPlaceholder')}
          disabled={loading}
          onKeyDown={(e) => e.key === 'Enter' && canImport && void handleImport()}
          className="flex-1 text-sm"
        />
        <Button
          variant="glass"
          size="sm"
          onClick={() => void handleImport()}
          disabled={!canImport}
          className="shrink-0 gap-1.5"
        >
          {loading && <Loader2 size={13} className="animate-spin" />}
          {t('resume.profileImport.import')}
        </Button>
      </div>
    </div>
  );
}
