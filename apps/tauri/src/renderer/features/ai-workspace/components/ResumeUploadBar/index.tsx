import { X } from 'lucide-react';

import { Button } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

export function ResumeUploadBar({
  fileName,
  onRemove,
}: {
  fileName: string;
  onRemove: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="mx-auto mb-3 flex max-w-3xl items-center gap-2 rounded-lg border border-brand-soft/20 bg-brand-soft/5 px-3 py-2">
      <span className="flex-1 truncate text-xs text-brand-soft/90">{fileName}</span>
      <Button
        onClick={onRemove}
        className="text-brand-soft/70 hover:text-brand-soft h-auto bg-transparent border-transparent p-0"
        aria-label={t('ai.removeResume')}
      >
        <X size={14} />
      </Button>
    </div>
  );
}
