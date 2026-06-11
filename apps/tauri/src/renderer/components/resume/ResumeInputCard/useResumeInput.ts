import { useEffect, useRef, useState } from 'react';

import type { DocumentRecord } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { useImportWithOcr } from '@/hooks/use-import-with-ocr';
import { useDocuments, useProfileImport, useSetDefaultDocument } from '@/services';

import { isProfileAuthError, isSupportedProfileUrl } from '../profile-url';

const MAX_BYTES = 25 * 1024 * 1024;

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
    source:
      raw.source ??
      (raw.name?.endsWith('.pdf') ? 'pdf' : raw.name?.endsWith('.docx') ? 'docx' : 'txt'),
  };
}

interface Params {
  value: string;
  onChange: (text: string) => void;
  onUpload: (file: File) => Promise<void>;
}

/** State + behavior for ResumeInputCard: saved docs, upload, paste, profile import. */
export function useResumeInput({ value, onChange, onUpload }: Params) {
  const { t } = useTranslation();
  const notify = useNotification();
  const fileRef = useRef<HTMLInputElement>(null);
  const savedBtnRef = useRef<HTMLButtonElement>(null);
  const savedMenuRef = useRef<HTMLDivElement>(null);

  const [expanded, setExpanded] = useState(true);
  const [inputMode, setInputMode] = useState<'upload' | 'paste'>('upload');
  const [dragging, setDragging] = useState(false);
  const [showSaved, setShowSaved] = useState(false);
  const [menuPos, setMenuPos] = useState({ top: 0, right: 0 });
  const [lastUploadedFile, setLastUploadedFile] = useState<File | null>(null);
  // Which saved resume is currently loaded into the editor (null when the text
  // came from an upload, paste, or profile import rather than a saved doc).
  const [selectedDocId, setSelectedDocId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [showUrlInput, setShowUrlInput] = useState(false);
  const [profileUrl, setProfileUrl] = useState('');

  const { data: rawDocsUnknown = [] } = useDocuments();
  const rawDocs = rawDocsUnknown as unknown as RawDoc[];
  const docs = rawDocs.map(normalise);
  const { importFile, review, clearReview } = useImportWithOcr();
  const setDefaultDocument = useSetDefaultDocument();
  const profileImport = useProfileImport();

  const hasSaved = docs.length > 0;
  const defaultDoc = docs.find((d) => d.isDefault) ?? docs[0];
  // Label the trigger with the loaded resume, falling back to the default.
  const triggerDoc = docs.find((d) => d.id === selectedDocId) ?? defaultDoc;

  // Close saved-menu on outside click
  useEffect(() => {
    if (!showSaved) return;
    const handler = (e: MouseEvent) => {
      if (
        savedBtnRef.current?.contains(e.target as Node) ||
        savedMenuRef.current?.contains(e.target as Node)
      )
        return;
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
    if (text) {
      onChange(text);
      setSelectedDocId(raw?._id ?? null);
    }
  }, [value, rawDocs, onChange]);

  /** Load a saved resume into the editor (does not change the default) */
  const handleSelectSaved = (doc: DocumentRecord) => {
    const raw = rawDocs.find((d) => d._id === doc.id);
    const text = raw?.text?.trim();
    if (text) onChange(text);
    setSelectedDocId(doc.id);
    setShowSaved(false);
    setLastUploadedFile(null);
  };

  /** Make a saved resume the default — keeps the menu open so the badge moves */
  const handleSetDefaultSaved = (doc: DocumentRecord) => {
    void setDefaultDocument.mutateAsync(doc.id);
  };

  /** Save the freshly-uploaded file to the document library */
  const handleSaveToLibrary = async (asDefault: boolean) => {
    if (!lastUploadedFile) return;
    setSaving(true);
    try {
      const result = await importFile(lastUploadedFile);
      if (result && typeof result === 'object' && 'id' in result && typeof result.id === 'string') {
        if (asDefault) await setDefaultDocument.mutateAsync(result.id);
        // The loaded text is now backed by this saved doc.
        setSelectedDocId(result.id);
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
    setSelectedDocId(null);
    await onUpload(file);
  };

  const profileUrlValid = isSupportedProfileUrl(profileUrl);

  const handleProfileUrlSubmit = async () => {
    const url = profileUrl.trim();
    if (!url || !profileUrlValid) return;
    try {
      const result = await profileImport.mutateAsync(url);
      if ('error' in result) {
        notify(
          isProfileAuthError(result.error) ? t('resumeInput.profileLoginRequired') : result.error,
          'error'
        );
        return;
      }
      onChange(result.text);
      setSelectedDocId(null);
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

  return {
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
    profileImportPending: profileImport.isPending,
    openSavedMenu,
    handleSelectSaved,
    handleSetDefaultSaved,
    handleSaveToLibrary,
    handleFileChange,
    handleProfileUrlSubmit,
    toggleUrlInput,
    review,
    clearReview,
  };
}
