import { ArrowLeft, ArrowRight, FileText, Loader2, Upload } from 'lucide-react';
import { motion } from 'motion/react';
import { useRef, useState } from 'react';

import type { DocumentRecord } from '@ajh/shared';
import { Button, FloatingIcon, useNotification } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { useDocuments, useImportDocument } from '@/services';
import { usePreferencesStore } from '@/store/preferences-store';

import { OnboardingStepWrapper } from '../components/OnboardingStepWrapper';

interface Props {
  onBack: () => void;
  onNext: () => void;
  direction: number;
  stepIndex: number;
  totalSteps: number;
}

export function ResumeStep({ onBack, onNext, direction, stepIndex, totalSteps }: Props) {
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
    <OnboardingStepWrapper
      direction={direction}
      stepIndex={stepIndex}
      totalSteps={totalSteps}
      onBack={onBack}
      onNext={onNext}
      canAdvance={hasResume}
    >
      {/* Header */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.05 }}
        className="mb-6 text-center"
      >
        <div className="mx-auto mb-4 relative flex justify-center">
          <FloatingIcon icon={FileText} size={26} />
        </div>
        <h2 className="text-xl font-semibold text-foreground">{t('onboarding.resume.title')}</h2>
        <p className="mt-2 text-sm text-foreground/50">{t('onboarding.resume.description')}</p>
      </motion.div>

      {/* Upload area */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.1 }}
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
      </motion.div>

      {/* Footer */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.1 }}
        className="flex items-center justify-between"
      >
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ArrowLeft size={14} /> {t('onboarding.back')}
        </Button>
        <Button
          variant="glass"
          size="md"
          onClick={onNext}
          className="transition-all duration-150 ease-out px-6 gap-2"
        >
          {hasResume ? t('onboarding.resume.next') : t('onboarding.resume.skip')}
          <ArrowRight size={14} />
        </Button>
      </motion.div>
    </OnboardingStepWrapper>
  );
}
