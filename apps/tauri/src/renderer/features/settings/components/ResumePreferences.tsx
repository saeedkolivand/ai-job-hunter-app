import { AlertCircle, FileText, Loader2, Sparkles, Trash2, Upload } from 'lucide-react';
import { motion } from 'motion/react';
import { useRef, useState } from 'react';

import type { DocumentRecord } from '@ajh/shared';
import { Button, GlassCard, useToast } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useDocuments, useImportDocument, useRemoveDocument } from '@/services';
import { usePreferencesStore, useResume } from '@/store/preferences-store';

export function ResumePreferences() {
  const { t } = useTranslation();
  const toast = useToast();
  const resume = useResume();
  const setResume = usePreferencesStore((state) => state.setResume);

  const { data: documentsRaw = [], isLoading } = useDocuments();
  // Rust serialises id as _id and created_at as createdAt — normalise here
  type RawDoc = Omit<DocumentRecord, 'id' | 'importedAt'> & {
    _id: string;
    createdAt: number;
    name?: string;
  };
  const documents: (DocumentRecord & { _rawSource?: string })[] = (documentsRaw as RawDoc[]).map(
    (d) => ({
      ...d,
      id: d._id,
      importedAt: d.createdAt,
      source: (d.source ??
        (d.name?.endsWith('.pdf')
          ? 'pdf'
          : d.name?.endsWith('.docx')
            ? 'docx'
            : 'txt')) as DocumentRecord['source'],
    })
  );
  const importDocument = useImportDocument();
  const removeDocument = useRemoveDocument();

  const [dragActive, setDragActive] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleFileUpload = async (file: File) => {
    if (!file) return;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      await importDocument.mutateAsync({ name: file.name, bytes, title: file.name });
      if (fileInputRef.current) fileInputRef.current.value = '';
      // Set as default only if none already selected
      if (!resume?.defaultId) {
        const first = (documentsRaw as RawDoc[])[0];
        const firstId = first?._id ?? first?.id;
        if (firstId) setResume({ defaultId: String(firstId), autoIndex: true, autoParse: true });
      }
      toast(t('settings.resume.uploaded'), 'success');
    } catch (err) {
      toast(err instanceof Error ? err.message : t('settings.resume.uploadFailed'), 'error');
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

  const handleSetDefault = (id: string) => {
    setResume({
      defaultId: id,
      autoIndex: resume?.autoIndex ?? true,
      autoParse: resume?.autoParse ?? true,
    });
  };

  const handleDelete = async (id: string) => {
    try {
      await removeDocument.mutateAsync(id);
      if (resume?.defaultId === id) {
        const next = documents.find((d) => d.id !== id);
        setResume({
          defaultId: next?.id,
          autoIndex: resume?.autoIndex ?? true,
          autoParse: resume?.autoParse ?? true,
        });
      }
      toast(t('settings.resume.removed'), 'success');
    } catch {
      toast(t('settings.resume.removeFailed'), 'error');
    }
  };

  const uploading = importDocument.isPending;

  const formatFileSize = (bytes?: number) => {
    if (!bytes) return '';
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  };

  return (
    <GlassCard>
      <div className="mb-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
        {t('settings.resume.title')}
      </div>
      <p className="mb-4 text-sm text-foreground/55">{t('settings.resume.description')}</p>

      {/* Upload area */}
      <div
        onDragEnter={handleDrag}
        onDragLeave={handleDrag}
        onDragOver={handleDrag}
        onDrop={handleDrop}
        onClick={() => !uploading && fileInputRef.current?.click()}
        className={cn(
          'relative mb-5 flex cursor-pointer flex-col items-center justify-center rounded-xl border-2 border-dashed p-8 transition-all',
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
            <Loader2 size={28} className="animate-spin text-brand-soft/60" />
            <div className="text-sm text-foreground/50">{t('settings.resume.uploading')}</div>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-3">
            <div
              className={cn(
                'rounded-full p-3 transition-colors',
                dragActive ? 'bg-brand/15' : 'bg-white/5'
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
            const isDefault = resume?.defaultId === doc.id;
            return (
              <motion.div
                key={doc.id}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={transition.normal}
                className={cn(
                  'flex items-center gap-3 rounded-xl border px-4 py-3 transition-all',
                  isDefault ? 'border-brand/30 bg-brand/8' : 'border-white/[0.06] bg-white/[0.02]'
                )}
              >
                <div
                  className={cn(
                    'flex h-9 w-9 shrink-0 items-center justify-center rounded-lg',
                    isDefault ? 'bg-brand/15' : 'bg-white/5'
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
                    size="sm"
                    onClick={() => handleSetDefault(doc.id)}
                    disabled={isDefault}
                    title={t('settings.resume.setDefault')}
                    className="text-foreground/30 hover:text-brand-soft disabled:opacity-0"
                  >
                    <Sparkles size={13} />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => void handleDelete(doc.id)}
                    disabled={removeDocument.isPending}
                    className="text-foreground/30 hover:text-red-400"
                  >
                    <Trash2 size={13} />
                  </Button>
                </div>
              </motion.div>
            );
          })}
        </div>
      )}
    </GlassCard>
  );
}
