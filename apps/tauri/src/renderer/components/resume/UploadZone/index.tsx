import { Check, Loader2, Upload } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { cn } from '@ajh/ui';

interface Props {
  uploading: boolean;
  dragging: boolean;
  lastUploadedFile: File | null;
  hasValue: boolean;
  onClick: () => void;
  onDragOver: (e: React.DragEvent) => void;
  onDragLeave: () => void;
  onDrop: (e: React.DragEvent) => void;
}

export function UploadZone({
  uploading,
  dragging,
  lastUploadedFile,
  hasValue,
  onClick,
  onDragOver,
  onDragLeave,
  onDrop,
}: Props) {
  const { t } = useTranslation();

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onClick();
        }
      }}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      className={cn(
        'flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed px-4 py-6 cursor-pointer transition-colors select-none',
        dragging
          ? 'border-brand/50 bg-brand/5'
          : lastUploadedFile && hasValue
            ? 'border-emerald-500/30 bg-emerald-500/5'
            : 'border-white/[0.08] hover:border-white/[0.15] hover:bg-white/[0.02]'
      )}
    >
      {uploading ? (
        <>
          <Loader2 size={20} className="animate-spin text-foreground/30" />
          <span className="text-[11px] text-foreground/40">{t('resumeInput.extracting')}</span>
        </>
      ) : lastUploadedFile && hasValue ? (
        <>
          <Check size={18} className="text-emerald-400" />
          <span className="text-[11px] text-foreground/70 text-center">
            {lastUploadedFile.name}
          </span>
          <span className="text-[10px] text-foreground/35">
            {t('resumeInput.uploadedClickToReplace')}
          </span>
        </>
      ) : (
        <>
          <Upload size={20} className="text-foreground/20" />
          <div className="text-center space-y-0.5">
            <p className="text-[11px] text-foreground/60">{t('resumeInput.dropOrClick')}</p>
            <p className="text-[10px] text-foreground/30">PDF, DOCX, TXT — max 25 MB</p>
          </div>
        </>
      )}
    </div>
  );
}
