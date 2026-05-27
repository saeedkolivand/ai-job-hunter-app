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
  Upload,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import type { DocumentRecord } from '@ajh/shared';
import { Button, cn, TextArea, useNotification } from '@ajh/ui';

import { useImportWithOcr } from '@/hooks/use-import-with-ocr';
import { useTranslation } from '@/lib/i18n';
import { useDocuments, useProfileImport, useSetDefaultDocument } from '@/services';

import { ProfileUrlInput } from './ProfileUrlInput';
import { SaveActions } from './SaveActions';
import { SavedResumeMenu } from './SavedResumeMenu';
import { UploadZone } from './UploadZone';

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

              <SavedResumeMenu
                show={showSaved}
                docs={docs}
                menuPos={menuPos}
                onSelect={handleSelectSaved}
              />
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
      <ProfileUrlInput
        show={showUrlInput && !disabled}
        url={profileUrl}
        onChange={setProfileUrl}
        onSubmit={handleProfileUrlSubmit}
        onCancel={() => {
          setShowUrlInput(false);
          setProfileUrl('');
        }}
        isPending={profileImport.isPending}
        isValid={profileUrlValid}
      />

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
            <UploadZone
              uploading={uploading}
              dragging={dragging}
              lastUploadedFile={lastUploadedFile}
              hasValue={!!value}
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
            />
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
      {lastUploadedFile && value && (
        <SaveActions
          fileName={lastUploadedFile.name}
          saving={saving}
          onSaveToLibrary={() => void handleSaveToLibrary(false)}
          onSetDefault={() => void handleSaveToLibrary(true)}
        />
      )}
    </div>
  );
}
