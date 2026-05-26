import { Link, Loader2 } from 'lucide-react';
import { useState } from 'react';

import { Button, Input, useNotification } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useImportDocument, useProfileImport } from '@/services';

interface Props {
  onImported?: (name: string) => void;
}

const LINKEDIN_PLACEHOLDER = 'https://www.linkedin.com/in/your-profile/';

export function ProfileUrlImport({ onImported }: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const [url, setUrl] = useState('');

  const profileImport = useProfileImport();
  const importDocument = useImportDocument();

  const isLinkedIn = url.toLowerCase().includes('linkedin.com/in/');
  const canImport = isLinkedIn && !profileImport.isPending && !importDocument.isPending;
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
      const name = result.name ? `${result.name} (LinkedIn).txt` : 'LinkedIn Profile.txt';

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
          placeholder={LINKEDIN_PLACEHOLDER}
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
          {loading ? <Loader2 size={13} className="animate-spin" /> : <Link size={13} />}
          {t('resume.profileImport.import')}
        </Button>
      </div>
      {url && !isLinkedIn && (
        <p className="text-xs text-amber-400/70">{t('resume.profileImport.linkedInOnly')}</p>
      )}
    </div>
  );
}
