/**
 * Resume input for AI Generate and Analyze pages.
 * Lets users: (1) pick a saved resume, (2) upload a new file, (3) paste text.
 * When a new file is uploaded it shows Save / Set-as-default actions.
 */
import {
  BookmarkCheck,
  Check,
  ChevronDown,
  ChevronUp,
  FileText,
  Loader2,
  Save,
  Sparkles,
  Upload,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import type { DocumentRecord } from '@ajh/shared';
import { Button, TextArea, useToast } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { useDocuments, useImportDocument } from '@/services';
import { usePreferencesStore, useResume } from '@/store/preferences-store';

const ACCEPT = '.pdf,.docx,.txt,.md,.markdown';
const MAX_BYTES = 25 * 1024 * 1024;

interface Props {
  /** Extracted resume text — controlled by parent */
  value: string;
  onChange: (text: string) => void;
  /** Called when the parent needs to extract text from a raw file */
  onUpload: (file: File) => Promise<void>;
  uploading: boolean;
  disabled?: boolean;
  placeholder?: string;
}

type RawDoc = Omit<DocumentRecord, 'id' | 'importedAt'> & {
  _id: string;
  createdAt: number;
  name?: string;
  text?: string; // returned by the backend but not in the shared TS type
};

function normalise(raw: RawDoc): DocumentRecord {
  return {
    ...raw,
    id: raw._id,
    importedAt: raw.createdAt,
    source: (raw.source ??
      (raw.name?.endsWith('.pdf')
        ? 'pdf'
        : raw.name?.endsWith('.docx')
          ? 'docx'
          : 'txt')) as DocumentRecord['source'],
  };
}

export function ResumeInputCard({
  value,
  onChange,
  onUpload,
  uploading,
  disabled,
  placeholder,
}: Props) {
  const { t } = useTranslation();
  const toast = useToast();
  const fileRef = useRef<HTMLInputElement>(null);

  const [expanded, setExpanded] = useState(true);
  const [showSaved, setShowSaved] = useState(false);
  const [lastUploadedFile, setLastUploadedFile] = useState<File | null>(null);
  const [saving, setSaving] = useState(false);

  const { data: rawDocsUnknown = [] } = useDocuments();
  const rawDocs = rawDocsUnknown as unknown as RawDoc[];
  const docs = rawDocs.map(normalise);
  const resumePref = useResume();
  const setResume = usePreferencesStore((s) => s.setResume);
  const importDocument = useImportDocument();

  const hasSaved = docs.length > 0;
  const defaultDoc = docs.find((d) => d.id === resumePref?.defaultId) ?? docs[0];

  // Auto-fill from default resume on first load (only if textarea is still empty)
  useEffect(() => {
    if (value) return;
    const raw = rawDocs.find((d) => d._id === resumePref?.defaultId) ?? rawDocs[0];
    const text = raw?.text?.trim();
    if (text) onChange(text);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rawDocs.length]);

  /** Load text from a saved document record into the textarea */
  const handleSelectSaved = (doc: DocumentRecord) => {
    const raw = rawDocs.find((d) => d._id === doc.id);
    const text = raw?.text?.trim();
    if (text) {
      onChange(text);
    }
    setResume({
      defaultId: doc.id,
      autoIndex: resumePref?.autoIndex ?? true,
      autoParse: resumePref?.autoParse ?? true,
    });
    setShowSaved(false);
    setLastUploadedFile(null);
    toast(t('resumeInput.selectedSaved', { name: doc.title }), 'success');
  };

  /** Save the freshly-uploaded file to the document library */
  const handleSaveToLibrary = async (asDefault: boolean) => {
    if (!lastUploadedFile) return;
    setSaving(true);
    try {
      const bytes = new Uint8Array(await lastUploadedFile.arrayBuffer());
      await importDocument.mutateAsync({
        name: lastUploadedFile.name,
        bytes,
        title: lastUploadedFile.name,
      });
      const saved = rawDocs[0];
      const id = saved?._id ?? saved?.id;
      if (asDefault && id) {
        setResume({ defaultId: String(id), autoIndex: true, autoParse: true });
      }
      toast(
        asDefault ? t('resumeInput.savedAsDefault') : t('resumeInput.savedToLibrary'),
        'success'
      );
      setLastUploadedFile(null);
    } catch {
      toast(t('resumeInput.saveFailed'), 'error');
    } finally {
      setSaving(false);
    }
  };

  const handleFileChange = async (file: File) => {
    if (file.size > MAX_BYTES) {
      toast(t('resumeInput.tooLarge'), 'error');
      return;
    }
    setLastUploadedFile(file);
    await onUpload(file);
  };

  return (
    <div
      className={cn(
        'glass-graphite glass-highlight rounded-xl overflow-hidden transition-colors',
        value && 'border-brand/20'
      )}
    >
      {/* Header row */}
      <div className="flex items-center justify-between px-3 py-2.5">
        <div className="flex items-center gap-2">
          <FileText size={13} className={value ? 'text-brand-soft' : 'text-foreground/30'} />
          <span className="text-xs font-medium text-foreground/70">{t('resumeInput.label')}</span>
          {value && <Check size={11} className="text-emerald-400" />}
        </div>
        <div className="flex items-center gap-1.5">
          {/* Pick saved */}
          {hasSaved && !disabled && (
            <div className="relative">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowSaved((v) => !v)}
                className="gap-1 text-[10px] text-foreground/45 hover:text-foreground/70 h-6 px-2"
              >
                <BookmarkCheck size={11} />
                {defaultDoc
                  ? defaultDoc.title.slice(0, 18) + (defaultDoc.title.length > 18 ? '…' : '')
                  : t('resumeInput.saved')}
                {showSaved ? <ChevronUp size={10} /> : <ChevronDown size={10} />}
              </Button>

              {showSaved && (
                <div className="absolute right-0 top-full z-50 mt-1 min-w-[200px] rounded-xl glass-elevated shadow-2xl overflow-hidden">
                  <div className="px-2 py-1.5 space-y-0.5 max-h-48 overflow-y-auto">
                    {docs.map((doc) => (
                      <button
                        key={doc.id}
                        onClick={() => handleSelectSaved(doc)}
                        className={cn(
                          'flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-xs transition-colors',
                          doc.id === resumePref?.defaultId
                            ? 'bg-brand/15 text-brand-soft'
                            : 'text-foreground/65 hover:bg-white/[0.05] hover:text-foreground/90'
                        )}
                      >
                        <FileText size={11} className="shrink-0" />
                        <span className="truncate flex-1">{doc.title}</span>
                        {doc.id === resumePref?.defaultId && <Sparkles size={9} />}
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Upload button */}
          {!disabled && (
            <>
              <input
                ref={fileRef}
                type="file"
                accept={ACCEPT}
                className="hidden"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) void handleFileChange(f);
                  e.target.value = '';
                }}
              />
              <Button
                variant="ghost"
                size="sm"
                onClick={() => fileRef.current?.click()}
                disabled={uploading}
                className="h-6 w-6 p-0 text-foreground/40 hover:text-foreground/70"
              >
                {uploading ? <Loader2 size={11} className="animate-spin" /> : <Upload size={11} />}
              </Button>
            </>
          )}

          {/* Collapse toggle */}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setExpanded((v) => !v)}
            className="h-6 w-6 p-0 text-foreground/25 hover:text-foreground/50"
          >
            {expanded ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
          </Button>
        </div>
      </div>

      {/* Text area */}
      {expanded && (
        <div className="px-3 pb-3">
          <TextArea
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder ?? t('resumeInput.placeholder')}
            disabled={disabled}
            rows={6}
            className="w-full resize-none bg-transparent text-xs text-foreground/80 placeholder:text-foreground/20 disabled:opacity-50"
          />
        </div>
      )}

      {/* Save actions — shown after a fresh file upload */}
      {lastUploadedFile && value && !saving && (
        <div className="flex items-center gap-2 border-t border-white/[0.05] px-3 py-2">
          <span className="flex-1 truncate text-[11px] text-foreground/40">
            {lastUploadedFile.name}
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void handleSaveToLibrary(false)}
            className="gap-1 text-[11px] text-foreground/45 hover:text-foreground/80 h-6 px-2"
          >
            <Save size={10} /> {t('resumeInput.saveToLibrary')}
          </Button>
          <Button
            variant="glass"
            size="sm"
            onClick={() => void handleSaveToLibrary(true)}
            className="gap-1 text-[11px] h-6 px-2 glow-subtle"
          >
            <Sparkles size={10} /> {t('resumeInput.setDefault')}
          </Button>
        </div>
      )}

      {saving && (
        <div className="flex items-center gap-2 border-t border-white/[0.05] px-3 py-2 text-[11px] text-foreground/40">
          <Loader2 size={10} className="animate-spin" /> {t('resumeInput.saving')}
        </div>
      )}
    </div>
  );
}
