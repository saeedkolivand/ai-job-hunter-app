import { AlertCircle, Download, FileText, Loader2, Sparkles, Trash2, Upload } from 'lucide-react';
import { motion } from 'motion/react';
import { useRef, useState } from 'react';

import type { ContactFieldConflict, DocumentRecord } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, cn, GlassCard, transition, useNotification } from '@ajh/ui';

import { ContactConflictModal } from '@/components/contact/ContactConflictModal';
import { ProfileUrlImport } from '@/features/resume/components/ProfileUrlImport';
import { useImportWithOcr } from '@/hooks/use-import-with-ocr';
import { exportTXT } from '@/lib/generate';
import { useDocuments, useRemoveDocument, useSetDefaultDocument } from '@/services';

export function ResumePreferences() {
  const { t } = useTranslation();
  const notify = useNotification();

  const { data: documentsRaw = [], isLoading } = useDocuments();
  // Rust serialises id as _id and created_at as createdAt — normalise here
  type RawDoc = Omit<DocumentRecord, 'id' | 'importedAt'> & {
    _id: string;
    createdAt: number;
    name?: string;
  };
  const rawDocs = documentsRaw as unknown as RawDoc[];
  const documents: DocumentRecord[] = rawDocs.map((d) => ({
    ...d,
    id: d._id,
    importedAt: d.createdAt,
    source:
      d.source ?? (d.name?.endsWith('.pdf') ? 'pdf' : d.name?.endsWith('.docx') ? 'docx' : 'txt'),
  }));
  const { importFile, isPending: uploading, isOcr } = useImportWithOcr();
  const removeDocument = useRemoveDocument();
  const setDefaultDocument = useSetDefaultDocument();

  const [dragActive, setDragActive] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Contact-mismatch follow-up: surfaced after a successful import when the
  // résumé's contact fields disagree with the saved profile. Never gates import.
  const [conflicts, setConflicts] = useState<ContactFieldConflict[]>([]);
  // Bumped per import so the modal remounts and re-seeds its rows cleanly.
  const [importKey, setImportKey] = useState(0);

  const handleFileUpload = async (file: File) => {
    if (!file) return;
    try {
      const result = await importFile(file);
      if (fileInputRef.current) fileInputRef.current.value = '';
      notify.success({ message: t('settings.resume.uploaded') });
      if (result.contactConflicts?.length) {
        setConflicts(result.contactConflicts);
        setImportKey((k) => k + 1);
      }
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : t('settings.resume.uploadFailed'),
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

  const handleSetDefault = async (id: string) => {
    try {
      await setDefaultDocument.mutateAsync(id);
      notify.success({ message: t('settings.resume.defaultSet') });
    } catch {
      notify.error({ message: t('settings.resume.defaultSetFailed') });
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await removeDocument.mutateAsync(id);
      notify.success({ message: t('settings.resume.removed') });
    } catch {
      notify.error({ message: t('settings.resume.removeFailed') });
    }
  };

  const handleDownload = (doc: DocumentRecord) => {
    const rawDoc = rawDocs.find((d) => d._id === doc.id);
    const text = (rawDoc as { text?: string })?.text || '';
    // Use the app's shared text-export util (validates, strips markdown, handles
    // the blob/anchor/revoke). Limitation: only the stored plain text is
    // available here, so the export is .txt — the original PDF/DOCX bytes aren't
    // persisted for re-download.
    try {
      exportTXT(text, `${doc.title.replace(/\.[^/.]+$/, '')}.txt`);
      notify.success({ message: t('settings.resume.downloaded') });
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : t('settings.resume.downloadFailed'),
      });
    }
  };

  return (
    <GlassCard>
      <div className="mb-2 text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
        {t('settings.resume.title')}
      </div>
      <p className="mb-4 text-sm text-foreground/55">{t('settings.resume.description')}</p>

      {/* Upload area */}
      <div
        role="button"
        tabIndex={0}
        onDragEnter={handleDrag}
        onDragLeave={handleDrag}
        onDragOver={handleDrag}
        onDrop={handleDrop}
        onClick={() => !uploading && fileInputRef.current?.click()}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            if (!uploading) fileInputRef.current?.click();
          }
        }}
        className={cn(
          'relative mb-5 flex cursor-pointer flex-col items-center justify-center rounded-xl border-2 border-dashed p-8 transition-all',
          dragActive
            ? 'border-brand/50 bg-brand/5'
            : 'border-foreground/10 bg-foreground/[0.03] hover:border-foreground/20 hover:bg-foreground/[0.05]',
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
            <Loader2 size={28} className="animate-spin text-brand-soft/60" />
            <div className="text-sm text-foreground/50">
              {isOcr ? t('settings.resume.scanning') : t('settings.resume.uploading')}
            </div>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-3">
            <div
              className={cn(
                'rounded-full p-3 transition-colors',
                dragActive ? 'bg-brand/15' : 'bg-foreground/[0.06]'
              )}
            >
              <Upload
                size={22}
                className={cn(
                  'transition-colors',
                  dragActive ? 'text-brand-soft' : 'text-foreground/35'
                )}
              />
            </div>
            <div className="text-sm text-foreground/60">{t('settings.resume.dragDrop')}</div>
            <div className="text-xs text-foreground/35">
              {t('settings.resume.orClick')} · PDF, DOC, DOCX, TXT
            </div>
          </div>
        )}
      </div>

      {/* LinkedIn / profile URL import */}
      <div className="mb-5">
        <ProfileUrlImport />
      </div>

      {/* Document list */}
      {isLoading ? (
        <div className="flex justify-center py-6">
          <Loader2 size={20} className="animate-spin text-foreground/20" />
        </div>
      ) : documents.length === 0 ? (
        <div className="flex flex-col items-center gap-3 py-8 text-center">
          <AlertCircle size={28} className="text-foreground/15" />
          <div className="text-sm text-foreground/35">{t('settings.resume.noResumes')}</div>
        </div>
      ) : (
        <div className="space-y-2">
          {documents.map((doc) => {
            const isDefault = doc.isDefault ?? false;
            return (
              <motion.div
                key={doc.id}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={transition.normal}
                className={cn(
                  'flex items-center gap-3 rounded-xl border px-4 py-3 transition-all',
                  isDefault
                    ? 'border-brand/30 bg-brand/8'
                    : 'border-foreground/10 bg-foreground/[0.03]'
                )}
              >
                <div
                  className={cn(
                    'flex h-9 w-9 shrink-0 items-center justify-center rounded-lg',
                    isDefault ? 'bg-brand/15' : 'bg-foreground/[0.06]'
                  )}
                >
                  <FileText
                    size={16}
                    className={isDefault ? 'text-brand-soft' : 'text-foreground/35'}
                  />
                </div>

                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span
                      className={cn(
                        'truncate text-sm font-medium',
                        isDefault ? 'text-foreground/90' : 'text-foreground/65'
                      )}
                    >
                      {doc.title}
                    </span>
                    {isDefault && (
                      <span className="flex shrink-0 items-center gap-1 rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-brand-soft">
                        <Sparkles size={8} /> {t('settings.resume.default')}
                      </span>
                    )}
                  </div>
                  <div className="mt-0.5 flex items-center gap-2 text-[11px] text-foreground/35">
                    <span className="uppercase">{doc.source}</span>
                    {doc.pages && <span>· {doc.pages}p</span>}
                    <span>· {new Date(doc.importedAt).toLocaleDateString('en-GB')}</span>
                  </div>
                </div>

                <div className="flex items-center gap-1">
                  <Button
                    variant="ghost"
                    onClick={() => void handleSetDefault(doc.id)}
                    disabled={isDefault || setDefaultDocument.isPending}
                    title={t('settings.resume.setDefault')}
                    className="h-8 w-8 p-0 text-foreground/30 hover:text-brand-soft disabled:opacity-0"
                  >
                    <Sparkles size={13} />
                  </Button>
                  <Button
                    variant="ghost"
                    onClick={() => handleDownload(doc)}
                    title={t('settings.resume.download')}
                    className="h-8 w-8 p-0 text-foreground/30 hover:text-blue-400"
                  >
                    <Download size={13} />
                  </Button>
                  <Button
                    variant="ghost"
                    onClick={() => void handleDelete(doc.id)}
                    disabled={removeDocument.isPending}
                    className="h-8 w-8 p-0 text-foreground/30 hover:text-red-400"
                  >
                    <Trash2 size={13} />
                  </Button>
                </div>
              </motion.div>
            );
          })}
        </div>
      )}

      <ContactConflictModal
        key={importKey}
        open={conflicts.length > 0}
        conflicts={conflicts}
        onClose={() => setConflicts([])}
      />
    </GlassCard>
  );
}
