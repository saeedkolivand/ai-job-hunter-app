import { ArrowLeft, ArrowRight, FileText, Loader2, Upload } from 'lucide-react';
import { motion } from 'motion/react';
import { useRef, useState } from 'react';

import type { DocumentRecord } from '@ajh/shared';
import { Button, useNotification } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useDocuments, useImportDocument } from '@/services';
import { usePreferencesStore } from '@/store/preferences-store';

interface Props {
  onBack: () => void;
  onNext: () => void;
  direction: number;
}

export function ResumeStep({ onBack, onNext, direction }: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [dragActive, setDragActive] = useState(false);

  const { data: documentsRaw = [] } = useDocuments();
  const documents = documentsRaw as DocumentRecord[];
  const importDocument = useImportDocument();
  const setResume = usePreferencesStore((s) => s.setResume);

  const uploading = importDocument.isPending;
  const hasResume = documents.length > 0;

  const handleFileUpload = async (file: File) => {
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      await importDocument.mutateAsync({ name: file.name, bytes, title: file.name });
      const first = (documentsRaw as (DocumentRecord & { _id?: string })[]).find(Boolean);
      const id = first?._id ?? first?.id;
      if (id) setResume({ defaultId: String(id), autoIndex: true, autoParse: true });
      notify(t('onboarding.resume.uploaded'), 'success');
    } catch (err) {
      notify(err instanceof Error ? err.message : t('onboarding.resume.uploadFailed'), 'error');
    }
  };

  const handleDrag = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragActive(e.type === 'dragenter' || e.type === 'dragover');
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragActive(false);
    const file = e.dataTransfer.files?.[0];
    if (file) void handleFileUpload(file);
  };

  return (
    <motion.div
      custom={direction}
      variants={{
        initial: (dir: number) => ({ opacity: 0, x: dir * 60 }),
        animate: { opacity: 1, x: 0 },
        exit: (dir: number) => ({ opacity: 0, x: dir * -60 }),
      }}
      initial="initial"
      animate="animate"
      exit="exit"
      transition={transition.modal}
      className="relative z-10 w-full max-w-lg px-4"
    >
      <div className="glass-modal rounded-2xl border border-white/[0.08] p-8 shadow-2xl">
        {/* Header */}
        <div className="mb-6 text-center">
          <div className="mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-brand/10 ring-1 ring-brand/20">
            <FileText size={26} className="text-brand-soft/70" />
          </div>
          <h2 className="text-xl font-semibold text-foreground">{t('onboarding.resume.title')}</h2>
          <p className="mt-2 text-sm text-foreground/50">{t('onboarding.resume.description')}</p>
        </div>

        {/* Upload area */}
        <div
          onDragEnter={handleDrag}
          onDragLeave={handleDrag}
          onDragOver={handleDrag}
          onDrop={handleDrop}
          onClick={() => !uploading && fileInputRef.current?.click()}
          className={cn(
            'mb-5 flex cursor-pointer flex-col items-center justify-center rounded-xl border-2 border-dashed p-8 transition-all',
            dragActive
              ? 'border-brand/50 bg-brand/5'
              : 'border-white/10 bg-white/[0.02] hover:border-white/20 hover:bg-white/[0.04]',
            uploading && 'cursor-default opacity-60'
          )}
        >
          <input
            ref={fileInputRef}
            type="file"
            accept=".pdf,.doc,.docx,.txt"
            className="hidden"
            onChange={(e) => e.target.files?.[0] && void handleFileUpload(e.target.files[0])}
          />
          {uploading ? (
            <div className="flex flex-col items-center gap-3">
              <Loader2 size={26} className="animate-spin text-brand-soft/60" />
              <p className="text-sm text-foreground/50">{t('onboarding.resume.uploading')}</p>
            </div>
          ) : hasResume ? (
            <div className="flex flex-col items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/15">
                <FileText size={20} className="text-emerald-400" />
              </div>
              <p className="text-sm font-medium text-foreground/80">{documents[0]?.title}</p>
              <p className="text-xs text-foreground/35">{t('onboarding.resume.replaceHint')}</p>
            </div>
          ) : (
            <div className="flex flex-col items-center gap-3">
              <div className={cn('rounded-full p-3', dragActive ? 'bg-brand/15' : 'bg-white/5')}>
                <Upload
                  size={22}
                  className={cn(
                    'transition-colors',
                    dragActive ? 'text-brand-soft' : 'text-foreground/35'
                  )}
                />
              </div>
              <p className="text-sm text-foreground/60">{t('onboarding.resume.dragDrop')}</p>
              <p className="text-xs text-foreground/35">PDF, DOC, DOCX, TXT · max 50 MB</p>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between">
          <Button variant="ghost" size="sm" onClick={onBack}>
            <ArrowLeft size={14} /> {t('onboarding.back')}
          </Button>
          <Button
            variant="glass"
            size="md"
            onClick={onNext}
            className={hasResume ? 'hover:glow-purple px-6 gap-2' : 'px-6 gap-2'}
          >
            {hasResume ? t('onboarding.continue') : t('onboarding.resume.skip')}
            <ArrowRight size={14} />
          </Button>
        </div>
      </div>
    </motion.div>
  );
}
