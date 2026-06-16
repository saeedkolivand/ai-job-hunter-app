import { Link2 } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, useNotification } from '@ajh/ui';

import { isProfileAuthError, isSupportedProfileUrl } from '@/components/resume/profile-url';
import { ProfileUrlInput } from '@/components/resume/ProfileUrlInput';
import { useImportDocument, useProfileImport } from '@/services';

interface Props {
  /** Called after a successful import + save to the document library. */
  onImported?: (result: { id?: string; name?: string }) => void;
}

/**
 * Import a resume from a public LinkedIn profile URL and save it to the document
 * library. Used in onboarding and Settings (contexts that need a saved document,
 * unlike ResumeInputCard which loads the text into the editor). No login is
 * required for public profiles; private ones surface a "log in" hint.
 */
export function ProfileUrlImport({ onImported }: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const profileImport = useProfileImport();
  const importDocument = useImportDocument();
  const [show, setShow] = useState(false);
  const [url, setUrl] = useState('');

  const valid = isSupportedProfileUrl(url);
  const pending = profileImport.isPending || importDocument.isPending;

  const reset = () => {
    setShow(false);
    setUrl('');
  };

  const handleSubmit = async () => {
    if (!url.trim() || !valid || pending) return;
    try {
      const result = await profileImport.mutateAsync(url.trim());
      if ('error' in result) {
        notify.error({
          message: isProfileAuthError(result.error)
            ? t('resumeInput.profileLoginRequired')
            : result.error,
        });
        return;
      }
      const title = result.name?.trim() || t('resumeInput.linkedinProfileTitle');
      const saved = (await importDocument.mutateAsync({
        name: `${title}.txt`,
        bytes: new TextEncoder().encode(result.text),
        title,
      })) as { id?: string };
      notify.success({ message: t('resumeInput.profileImported') });
      onImported?.({ id: saved?.id, name: result.name });
      reset();
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : t('resumeInput.profileImportFailed'),
      });
    }
  };

  return (
    <div className="overflow-hidden rounded-xl border border-foreground/10 bg-foreground/[0.03]">
      <Button
        variant="ghost"
        onClick={() => {
          setShow((v) => !v);
          setUrl('');
        }}
        className="flex h-auto w-full items-center justify-start gap-2 px-3 py-2.5 text-xs font-medium text-foreground/60 hover:text-foreground/85"
      >
        <Link2 size={13} className={show ? 'text-brand-soft' : 'text-foreground/35'} />
        {t('resumeInput.importFromLinkedin')}
      </Button>
      <ProfileUrlInput
        show={show}
        url={url}
        onChange={setUrl}
        onSubmit={handleSubmit}
        onCancel={reset}
        isPending={pending}
        isValid={valid}
      />
    </div>
  );
}
