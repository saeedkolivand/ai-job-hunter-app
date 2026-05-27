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
  ClipboardPaste,
  FileText,
  Link,
  Loader2,
  Save,
  Sparkles,
  Upload,
  X,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import type { DocumentRecord } from '@ajh/shared';
import { Button, cn, TextArea, useNotification } from '@ajh/ui';

import { useImportWithOcr } from '@/hooks/use-import-with-ocr';
import { useTranslation } from '@/lib/i18n';
import { useDocuments, useProfileImport, useSetDefaultDocument } from '@/services';

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
  const notify = useNotification();
  const fileRef = useRef<HTMLInputElement>(null);

  const savedBtnRef = useRef<HTMLButtonElement>(null);
  const [expanded, setExpanded] = useState(true);
  const [inputMode, setInputMode] = useState<'upload' | 'paste'>('upload');
  const [dragging, setDragging] = useState(false);
  const [showSaved, setShowSaved] = useState(false);
  const [menuPos, setMenuPos] = useState({ top: 0, right: 0 });
  const [lastUploadedFile, setLastUploadedFile] = useState<File | null>(null);
  const [saving, setSaving] = useState(false);
  const [showUrlInput, setShowUrlInput] = useState(false);
  const [profileUrl, setProfileUrl] = useState('');
  const urlInputRef = useRef<HTMLInputElement>(null);

  const { data: rawDocsUnknown = [] } = useDocuments();
  const rawDocs = rawDocsUnknown as unknown as RawDoc[];
  const docs = rawDocs.map(normalise);
  const { importFile } = useImportWithOcr();
  const setDefaultDocument = useSetDefaultDocument();
  const profileImport = useProfileImport();

  const hasSaved = docs.length > 0;
  const defaultDoc = docs.find((d) => d.isDefault) ?? docs[0];

  // Close saved-menu on outside click
  useEffect(() => {
    if (!showSaved) return;
    const handler = (e: MouseEvent) => {
      if (savedBtnRef.current?.contains(e.target as Node)) return;
      setShowSaved(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [showSaved]);

  const openSavedMenu = () => {
    if (!savedBtnRef.current) return;
    const rect = savedBtnRef.current.getBoundingClientRect();
    setMenuPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
    setShowSaved((v) => !v);
  };

  // Auto-fill from default resume on first load (only if textarea is still empty)
  useEffect(() => {
    if (value) return;
    const raw = rawDocs.find((d) => d.isDefault) ?? rawDocs[0];
    const text = raw?.text?.trim();
    if (text) onChange(text);
  }, [value, rawDocs, onChange]);

  /** Load text from a saved document record into the textarea */
  const handleSelectSaved = async (doc: DocumentRecord) => {
    const raw = rawDocs.find((d) => d._id === doc.id);
    const text = raw?.text?.trim();
    if (text) {
      onChange(text);
    }
    // Set as default in backend
    await setDefaultDocument.mutateAsync(doc.id);
    setShowSaved(false);
    setLastUploadedFile(null);
    notify(t('resumeInput.selectedSaved', { name: doc.title }), 'success');
  };

  /** Save the freshly-uploaded file to the document library */
  const handleSaveToLibrary = async (asDefault: boolean) => {
    if (!lastUploadedFile) return;
    setSaving(true);
    try {
      const result = await importFile(lastUploadedFile);
      if (
        asDefault &&
        result &&
        typeof result === 'object' &&
        'id' in result &&
        typeof result.id === 'string'
      ) {
        await setDefaultDocument.mutateAsync(result.id);
      }
      notify(
        asDefault ? t('resumeInput.savedAsDefault') : t('resumeInput.savedToLibrary'),
        'success'
      );
      setLastUploadedFile(null);
    } catch {
      notify(t('resumeInput.saveFailed'), 'error');
    } finally {
      setSaving(false);
    }
  };

  const handleFileChange = async (file: File) => {
    if (file.size > MAX_BYTES) {
      notify(t('resumeInput.tooLarge'), 'error');
      return;
    }
    setLastUploadedFile(file);
    await onUpload(file);
  };

  const isSupportedProfileUrl = (u: string) => u.toLowerCase().includes('linkedin.com/in/');
  const profileUrlValid = isSupportedProfileUrl(profileUrl);

  const handleProfileUrlSubmit = async () => {
    const url = profileUrl.trim();
    if (!url || !profileUrlValid) return;
    try {
      const result = await profileImport.mutateAsync(url);
      if ('error' in result) {
        notify(result.error, 'error');
        return;
      }
      onChange(result.text);
      setProfileUrl('');
      setShowUrlInput(false);
      notify(t('resumeInput.profileImported'), 'success');
    } catch (err) {
      notify(err instanceof Error ? err.message : t('resumeInput.profileImportFailed'), 'error');
    }
  };

  const toggleUrlInput = () => {
    setShowUrlInput((v) => !v);
    setProfileUrl('');
    setTimeout(() => urlInputRef.current?.focus(), 50);
  };

  return (
    <div
      className={cn(
        'glass-graphite glass-highlight rounded-xl transition-colors',
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
            <>
              <Button
                ref={savedBtnRef}
                variant="ghost"
                size="sm"
                onClick={openSavedMenu}
                className="gap-1 text-[10px] text-foreground/45 hover:text-foreground/70 h-6 px-2"
              >
                <BookmarkCheck size={11} />
                {defaultDoc
                  ? defaultDoc.title.slice(0, 18) + (defaultDoc.title.length > 18 ? '…' : '')
                  : t('resumeInput.saved')}
                {showSaved ? <ChevronUp size={10} /> : <ChevronDown size={10} />}
              </Button>

              {showSaved &&
                createPortal(
                  <div
                    style={{
                      position: 'fixed',
                      top: menuPos.top,
                      right: menuPos.right,
                      zIndex: 9999,
                    }}
                    className="min-w-[200px] rounded-xl glass-elevated shadow-2xl overflow-hidden"
                  >
                    <div className="px-2 py-1.5 space-y-0.5 max-h-48 overflow-y-auto">
                      {docs.map((doc) => (
                        <button
                          key={doc.id}
                          onClick={() => void handleSelectSaved(doc)}
                          className={cn(
                            'flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-xs transition-colors',
                            doc.isDefault
                              ? 'bg-brand/15 text-brand-soft'
                              : 'text-foreground/65 hover:bg-white/[0.05] hover:text-foreground/90'
                          )}
                        >
                          <FileText size={11} className="shrink-0" />
                          <span className="truncate flex-1">{doc.title}</span>
                          {doc.isDefault && <Sparkles size={9} />}
                        </button>
                      ))}
                    </div>
                  </div>,
                  document.body
                )}
            </>
          )}

          {/* Hidden file input — triggered by drop zone and saved-select */}
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

          {/* Profile URL import button */}
          {!disabled && (
            <Button
              variant="ghost"
              size="sm"
              onClick={toggleUrlInput}
              className={cn(
                'h-6 w-6 p-0 hover:text-foreground/70',
                showUrlInput ? 'text-brand-soft' : 'text-foreground/40'
              )}
              title={t('resumeInput.pasteProfileUrl')}
            >
              <Link size={11} />
            </Button>
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

      {/* Profile URL input panel */}
      {showUrlInput && !disabled && (
        <div className="flex flex-col border-t border-white/[0.05]">
          <div className="flex items-center gap-2 px-3 py-2">
            <input
              ref={urlInputRef}
              type="url"
              value={profileUrl}
              onChange={(e) => setProfileUrl(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void handleProfileUrlSubmit();
                if (e.key === 'Escape') {
                  setShowUrlInput(false);
                  setProfileUrl('');
                }
              }}
              placeholder={t('resumeInput.profileUrlPlaceholder')}
              disabled={profileImport.isPending}
              className="flex-1 bg-transparent text-xs text-foreground/80 placeholder:text-foreground/30 outline-none disabled:opacity-50"
            />
            {profileImport.isPending ? (
              <Loader2 size={11} className="shrink-0 animate-spin text-foreground/40" />
            ) : (
              <>
                <Button
                  variant="glass"
                  size="sm"
                  onClick={() => void handleProfileUrlSubmit()}
                  disabled={!profileUrlValid}
                  className="h-6 px-2 text-[11px] gap-1"
                >
                  {t('resumeInput.profileUrlImport')}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setShowUrlInput(false);
                    setProfileUrl('');
                  }}
                  className="h-6 w-6 p-0 text-foreground/30 hover:text-foreground/60"
                >
                  <X size={11} />
                </Button>
              </>
            )}
          </div>
          {profileUrl && !profileUrlValid && (
            <p className="px-3 pb-2 text-[10px] text-amber-400/70">
              {t('resumeInput.profileUrlUnsupported')}
            </p>
          )}
        </div>
      )}

      {/* Mode switcher + content */}
      {expanded && !disabled && (
        <div className="px-3 pb-3 space-y-2">
          {/* Segmented control */}
          <div className="flex items-center gap-1 rounded-lg bg-white/[0.04] p-0.5 w-fit">
            <button
              onClick={() => setInputMode('upload')}
              className={cn(
                'flex items-center gap-1.5 rounded-md px-3 py-1 text-[11px] font-medium transition-all',
                inputMode === 'upload'
                  ? 'bg-white/[0.08] text-foreground/90 shadow-sm'
                  : 'text-foreground/40 hover:text-foreground/60'
              )}
            >
              <Upload size={10} />
              {t('resumeInput.modeUpload')}
            </button>
            <button
              onClick={() => setInputMode('paste')}
              className={cn(
                'flex items-center gap-1.5 rounded-md px-3 py-1 text-[11px] font-medium transition-all',
                inputMode === 'paste'
                  ? 'bg-white/[0.08] text-foreground/90 shadow-sm'
                  : 'text-foreground/40 hover:text-foreground/60'
              )}
            >
              <ClipboardPaste size={10} />
              {t('resumeInput.modePaste')}
            </button>
          </div>

          {/* Upload zone */}
          {inputMode === 'upload' && (
            <div
              onClick={() => fileRef.current?.click()}
              onDragOver={(e) => {
                e.preventDefault();
                setDragging(true);
              }}
              onDragLeave={() => setDragging(false)}
              onDrop={(e) => {
                e.preventDefault();
                setDragging(false);
                const f = e.dataTransfer.files[0];
                if (f) void handleFileChange(f);
              }}
              className={cn(
                'flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed px-4 py-6 cursor-pointer transition-colors select-none',
                dragging
                  ? 'border-brand/50 bg-brand/5'
                  : lastUploadedFile && value
                    ? 'border-emerald-500/30 bg-emerald-500/5'
                    : 'border-white/[0.08] hover:border-white/[0.15] hover:bg-white/[0.02]'
              )}
            >
              {uploading ? (
                <>
                  <Loader2 size={20} className="animate-spin text-foreground/30" />
                  <span className="text-[11px] text-foreground/40">
                    {t('resumeInput.extracting')}
                  </span>
                </>
              ) : lastUploadedFile && value ? (
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
          )}

          {/* Paste / edit text area */}
          {inputMode === 'paste' && (
            <TextArea
              value={value}
              onChange={(e) => onChange(e.target.value)}
              placeholder={placeholder ?? t('resumeInput.placeholder')}
              rows={6}
              className="w-full resize-none bg-transparent text-xs text-foreground/80 placeholder:text-foreground/20"
            />
          )}
        </div>
      )}

      {/* Read-only textarea when card is disabled */}
      {expanded && disabled && (
        <div className="px-3 pb-3">
          <TextArea
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder ?? t('resumeInput.placeholder')}
            disabled
            rows={6}
            className="w-full resize-none bg-transparent text-xs text-foreground/80 placeholder:text-foreground/20 opacity-50"
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
            className="gap-1 text-[11px] h-6 px-2 ring-1 ring-brand/20"
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
