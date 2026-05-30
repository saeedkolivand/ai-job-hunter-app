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

import { Button, cn, TextArea } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import { ProfileUrlInput } from '../ProfileUrlInput';
import { ResumeReviewPanel } from '../ResumeReviewPanel';
import { SaveActions } from '../SaveActions';
import { SavedResumeMenu } from '../SavedResumeMenu';
import { UploadZone } from '../UploadZone';
import { useResumeInput } from './useResumeInput';

const ACCEPT = '.pdf,.docx,.txt,.md,.markdown,.html,.htm,.rtf';

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

export function ResumeInputCard({
  value,
  onChange,
  onUpload,
  uploading,
  disabled,
  placeholder,
}: Props) {
  const { t } = useTranslation();
  const {
    fileRef,
    savedBtnRef,
    savedMenuRef,
    expanded,
    setExpanded,
    inputMode,
    setInputMode,
    dragging,
    setDragging,
    showSaved,
    menuPos,
    lastUploadedFile,
    selectedDocId,
    saving,
    showUrlInput,
    setShowUrlInput,
    profileUrl,
    setProfileUrl,
    docs,
    hasSaved,
    triggerDoc,
    profileUrlValid,
    profileImportPending,
    openSavedMenu,
    handleSelectSaved,
    handleSetDefaultSaved,
    handleSaveToLibrary,
    handleFileChange,
    handleProfileUrlSubmit,
    toggleUrlInput,
    review,
    clearReview,
  } = useResumeInput({ value, onChange, onUpload });

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
                {triggerDoc
                  ? triggerDoc.title.slice(0, 18) + (triggerDoc.title.length > 18 ? '…' : '')
                  : t('resumeInput.saved')}
                {showSaved ? <ChevronUp size={10} /> : <ChevronDown size={10} />}
              </Button>

              <SavedResumeMenu
                show={showSaved}
                docs={docs}
                menuPos={menuPos}
                selectedId={selectedDocId}
                onSelect={handleSelectSaved}
                onSetDefault={handleSetDefaultSaved}
                menuRef={savedMenuRef}
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
        isPending={profileImportPending}
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

      {/* Structured-extraction review — shown when a saved import needs a look */}
      {review?.reviewRequired && (
        <div className="px-3 pb-3">
          <ResumeReviewPanel review={review} onDismiss={clearReview} />
        </div>
      )}
    </div>
  );
}
