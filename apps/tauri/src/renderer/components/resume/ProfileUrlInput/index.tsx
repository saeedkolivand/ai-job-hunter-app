import { Loader2, X } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, Input } from '@ajh/ui';

interface Props {
  show: boolean;
  url: string;
  onChange: (url: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
  isPending: boolean;
  isValid: boolean;
}

export function ProfileUrlInput({
  show,
  url,
  onChange,
  onSubmit,
  onCancel,
  isPending,
  isValid,
}: Props) {
  const { t } = useTranslation();

  if (!show) return null;

  return (
    <div className="flex flex-col border-t border-white/[0.05]">
      <div className="flex items-center gap-2 px-3 py-2">
        <Input
          variant="unstyled"
          type="url"
          value={url}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void onSubmit();
            if (e.key === 'Escape') onCancel();
          }}
          placeholder={t('resumeInput.profileUrlPlaceholder')}
          disabled={isPending}
          className="flex-1 bg-transparent text-xs text-foreground/80 placeholder:text-foreground/30 outline-none disabled:opacity-50"
        />
        {isPending ? (
          <Loader2 size={11} className="shrink-0 animate-spin text-foreground/40" />
        ) : (
          <>
            <Button
              variant="glass"
              onClick={() => void onSubmit()}
              disabled={!isValid}
              className="h-6 px-2 text-[11px] gap-1"
            >
              {t('resumeInput.profileUrlImport')}
            </Button>
            <Button
              variant="ghost"
              onClick={onCancel}
              className="h-6 w-6 p-0 text-foreground/30 hover:text-foreground/60"
            >
              <X size={11} />
            </Button>
          </>
        )}
      </div>
      {url && !isValid && (
        <p className="px-3 pb-2 text-[10px] text-amber-400/70">
          {t('resumeInput.profileUrlUnsupported')}
        </p>
      )}
    </div>
  );
}
