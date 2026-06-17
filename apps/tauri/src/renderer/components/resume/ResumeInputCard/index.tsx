/**
 * Resume input for AI Generate and Analyze pages.
 *
 * Resting: shows the active resume as a chip with a ✓ and a "Change" control.
 * Expanded: inline progressive disclosure — upload (auto-saves to the library),
 * paste (with a Save-to-library link), and a labelled LinkedIn URL import.
 */
import { Check, ChevronDown, ChevronUp, FileText } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, TextArea } from '@ajh/ui';

import { ProfileUrlInput } from '../ProfileUrlInput';
import { ResumeReviewPanel } from '../ResumeReviewPanel';
import { SavedResumeMenu } from '../SavedResumeMenu';
import { UploadZone } from '../UploadZone';
import { useResumeInput } from './useResumeInput';

const ACCEPT = '.pdf,.docx,.txt,.md,.markdown,.html,.htm,.rtf';

interface Props {
  /** Extracted resume text — controlled by parent */
  value: string;
  onChange: (text: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

export function ResumeInputCard({ value, onChange, disabled, placeholder }: Props) {
  const { t } = useTranslation();
  const {
    fileRef,
    savedBtnRef,
    savedMenuRef,
    expanded,
    setExpanded,
    dragging,
    setDragging,
    showSaved,
    menuPos,
    selectedDocId,
    showUrlInput,
    setShowUrlInput,
    profileUrl,
    setProfileUrl,
    docs,
    hasSaved,
    activeDoc,
    uploading,
    scanning,
    profileUrlValid,
    profileImportPending,
    openSavedMenu,
    handleSelectSaved,
    handleSetDefaultSaved,
    handleRemove,
    handleFileChange,
    handleSavePaste,
    handleProfileUrlSubmit,
    toggleUrlInput,
    review,
    clearReview,
  } = useResumeInput({ value, onChange });

  // Label for the resting chip: the loaded doc's title, else a generic label
  // when the text came from a paste / profile import (no backing doc).
  const chipLabel = activeDoc?.title ?? t('resumeInput.activeResume');

  // Show the add-options view when explicitly expanded, or derive it for the
  // genuinely-empty card (no saved docs and no text) — derived from live props
  // so it survives the async first render where the docs cache is still empty.
  const showAddOptions = expanded || (!hasSaved && !value);

  return (
    <div
      className={cn(
        'glass-graphite glass-highlight rounded-xl transition-colors',
        value && 'border-brand/20'
      )}
    >
      {/* Hidden file input — triggered by drop zone */}
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

      {/* RESTING — active resume chip */}
      {!showAddOptions && !disabled && (
        <div className="flex items-center gap-2 px-3 py-2.5">
          <FileText size={13} className={value ? 'text-brand-soft' : 'text-foreground/30'} />
          <span className="flex-1 min-w-0 truncate text-xs font-medium text-foreground/70">
            {chipLabel}
          </span>
          {value && <Check size={11} className="shrink-0 text-emerald-400" />}

          {hasSaved && !disabled && (
            <Button
              ref={savedBtnRef}
              variant="ghost"
              onClick={openSavedMenu}
              aria-label={t('resumeInput.saved')}
              className="h-6 w-6 shrink-0 p-0 text-foreground/25 hover:text-foreground/60"
            >
              {showSaved ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            </Button>
          )}

          {!disabled && (
            <Button
              variant="ghost"
              onClick={() => setExpanded(true)}
              className="h-6 shrink-0 px-2 text-[10px] text-foreground/45 hover:text-foreground/70"
            >
              {t('resumeInput.change')}
            </Button>
          )}

          <SavedResumeMenu
            show={showSaved}
            docs={docs}
            menuPos={menuPos}
            selectedId={selectedDocId}
            onSelect={handleSelectSaved}
            onSetDefault={handleSetDefaultSaved}
            onRemove={handleRemove}
            menuRef={savedMenuRef}
          />
        </div>
      )}

      {/* EXPANDED — inline add options (upload / paste / LinkedIn URL) */}
      {showAddOptions && !disabled && (
        <div className="space-y-3 px-3 py-3">
          {value && (
            <div className="flex justify-end">
              <Button
                variant="ghost"
                onClick={() => setExpanded(false)}
                aria-label={t('resumeInput.collapse')}
                className="h-6 w-6 p-0 text-foreground/25 hover:text-foreground/50"
              >
                <ChevronUp size={12} />
              </Button>
            </div>
          )}

          {/* Upload */}
          <UploadZone
            uploading={uploading}
            scanning={scanning}
            dragging={dragging}
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

          {/* Paste */}
          <div className="space-y-1.5">
            <span className="text-[11px] font-medium text-foreground/55">
              {t('resumeInput.pasteSection')}
            </span>
            <TextArea
              value={value}
              onChange={(e) => onChange(e.target.value)}
              placeholder={placeholder ?? t('resumeInput.placeholder')}
              rows={6}
              className="w-full resize-none bg-transparent text-xs text-foreground/80 placeholder:text-foreground/20"
            />
            {value.trim() && (
              <div className="flex justify-end">
                <Button
                  variant="ghost"
                  onClick={() => void handleSavePaste()}
                  className="h-6 px-2 text-[10px] text-foreground/45 hover:text-foreground/70"
                >
                  {t('resumeInput.saveToLibrary')}
                </Button>
              </div>
            )}
          </div>

          {/* LinkedIn URL import */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <span className="text-[11px] font-medium text-foreground/55">
                {t('resumeInput.importFromLinkedin')}
              </span>
              {!showUrlInput && (
                <Button
                  variant="ghost"
                  onClick={toggleUrlInput}
                  className="h-6 px-2 text-[10px] text-foreground/45 hover:text-foreground/70"
                >
                  {t('resumeInput.profileUrlImport')}
                </Button>
              )}
            </div>
            <ProfileUrlInput
              show={showUrlInput}
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
          </div>
        </div>
      )}

      {/* Read-only textarea when the card is disabled */}
      {disabled && (
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

      {/* Structured-extraction review — the panel renders null when nothing needs a look */}
      {review && (
        <div className="px-3 pb-3">
          <ResumeReviewPanel review={review} onDismiss={clearReview} />
        </div>
      )}
    </div>
  );
}
