import { ArrowLeft, ArrowRight, FileText, Loader2, Upload } from 'lucide-react';
import { motion } from 'motion/react';
import { useRef, useState } from 'react';

import type { ContactFieldConflict, DocumentRecord } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, cn, FloatingIcon, useNotification, withDelay } from '@ajh/ui';

import { ContactConflictModal } from '@/components/contact/ContactConflictModal';
import { ProfileUrlImport } from '@/features/resume/components/ProfileUrlImport';
import { useImportWithOcr } from '@/hooks/use-import-with-ocr';
import { useDocuments } from '@/services';
import { usePreferencesStore } from '@/store/preferences-store';

import { OnboardingStepWrapper } from '../../components/OnboardingStepWrapper';

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
  const documents = documentsRaw;
  const { importFile, isPending: uploading, isOcr } = useImportWithOcr();
  const setResume = usePreferencesStore((s) => s.setResume);

  // Contact-mismatch follow-up — only fires if the user already saved contact
  // fields (e.g. revisiting onboarding) that disagree with the imported résumé.
  const [conflicts, setConflicts] = useState<ContactFieldConflict[]>([]);
  // Bumped per import so the modal remounts and re-seeds its rows cleanly.
  const [importKey, setImportKey] = useState(0);

  const hasResume = documents.length > 0;

  const handleFileUpload = async (file: File) => {
    try {
      const result = await importFile(file);
      const first = (documentsRaw as (DocumentRecord & { _id?: string })[]).find(Boolean);
      const id = first?._id ?? first?.id;
      if (id) setResume({ defaultId: String(id), autoIndex: true, autoParse: true });
      notify.success({ message: t('onboarding.resume.uploaded') });
      if (result.contactConflicts?.length) {
        setConflicts(result.contactConflicts);
        setImportKey((k) => k + 1);
      }
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : t('onboarding.resume.uploadFailed'),
      });
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
        transition={withDelay(0.05)}
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
        transition={withDelay(0.1)}
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
            <p className="text-sm text-foreground/50">
              {isOcr ? t('settings.resume.scanning') : t('onboarding.resume.uploading')}
            </p>
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

      {/* LinkedIn import */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.15)}
        className="mb-5"
      >
        <ProfileUrlImport
          onImported={({ id }) => {
            if (id) setResume({ defaultId: id, autoIndex: true, autoParse: true });
          }}
        />
      </motion.div>

      {/* Footer */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.1)}
        className="flex items-center justify-between"
      >
        <Button variant="ghost" onClick={onBack}>
          <ArrowLeft size={14} /> {t('onboarding.back')}
        </Button>
        <Button
          variant="primary"
          size="md"
          onClick={onNext}
          className="transition-all duration-150 ease-out px-6 gap-2"
        >
          {hasResume ? t('onboarding.resume.next') : t('onboarding.resume.skip')}
          <ArrowRight size={14} />
        </Button>
      </motion.div>

      <ContactConflictModal
        key={importKey}
        open={conflicts.length > 0}
        conflicts={conflicts}
        onClose={() => setConflicts([])}
      />
    </OnboardingStepWrapper>
  );
}
