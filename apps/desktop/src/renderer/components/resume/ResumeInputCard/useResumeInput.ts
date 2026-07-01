import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import type { DocumentRecord } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { useImportWithOcr } from '@/hooks/use-import-with-ocr';
import { isRawDoc, normalise } from '@/lib/doc-record';
import { exportTXT } from '@/lib/generate';
import { useAppClient } from '@/providers/AppClientProvider';
import {
  keys,
  useDocuments,
  useProfileImport,
  useRemoveDocument,
  useSetDefaultDocument,
} from '@/services';

import { isProfileAuthError, isSupportedProfileUrl } from '../profile-url';

const MAX_BYTES = 25 * 1024 * 1024;

interface Params {
  value: string;
  onChange: (text: string) => void;
}

/** State + behavior for ResumeInputCard: saved docs, upload, paste, profile import. */
export function useResumeInput({ value, onChange }: Params) {
  const { t } = useTranslation();
  const notify = useNotification();
  const api = useAppClient();
  const queryClient = useQueryClient();
  const fileRef = useRef<HTMLInputElement>(null);
  const savedBtnRef = useRef<HTMLButtonElement>(null);
  const savedMenuRef = useRef<HTMLDivElement>(null);

  const { data: rawDocsUnknown = [] } = useDocuments();
  const rawDocs = (Array.isArray(rawDocsUnknown) ? (rawDocsUnknown as unknown[]) : []).filter(
    isRawDoc
  );
  const docs = rawDocs.map(normalise);

  // Starts collapsed; the component derives the empty-state expansion from live
  // props (the React Query cache is empty on the first synchronous render, so a
  // lazy initializer would lock returning users into the expanded view).
  const [expanded, setExpanded] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [showSaved, setShowSaved] = useState(false);
  const [menuPos, setMenuPos] = useState({ top: 0, right: 0 });
  // Which saved resume is currently loaded into the editor (null when the text
  // came from an upload, paste, or profile import rather than a saved doc).
  const [selectedDocId, setSelectedDocId] = useState<string | null>(null);
  const [showUrlInput, setShowUrlInput] = useState(false);
  const [profileUrl, setProfileUrl] = useState('');

  const { importFile, isOcr, isPending, review, clearReview } = useImportWithOcr();
  const setDefaultDocument = useSetDefaultDocument();
  const removeDocument = useRemoveDocument();
  const profileImport = useProfileImport();

  const hasSaved = docs.length > 0;
  const defaultDoc = docs.find((d) => d.isDefault) ?? docs[0];
  // The doc backing the current editor text, falling back to the default.
  const triggerDoc = docs.find((d) => d.id === selectedDocId) ?? defaultDoc;
  const activeDoc = triggerDoc;

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
    setExpanded(false);
  };

  /** Make a saved resume the default — keeps the menu open so the badge moves */
  const handleSetDefaultSaved = (doc: DocumentRecord) => {
    void setDefaultDocument.mutateAsync(doc.id);
  };

  /** Remove a saved resume from the library (no confirmation modal — the menu
   *  row handles inline confirm). Clears the selection if it was the active doc. */
  const handleRemove = (doc: DocumentRecord) => {
    void removeDocument.mutateAsync(doc.id);
    if (doc.id === selectedDocId) {
      setSelectedDocId(null);
      onChange('');
      setExpanded(true);
    }
  };

  const handleDownload = (doc: DocumentRecord) => {
    const raw = rawDocs.find((d) => d._id === doc.id);
    const text = raw?.text?.trim() ?? '';
    try {
      exportTXT(text, `${doc.title.replace(/\.[^/.]+$/, '')}.txt`);
      notify.success({ message: t('resumeInput.downloaded') });
    } catch {
      // exportTXT throws English-only messages; show the localized key instead
      // of leaking raw exception text into a non-English locale's toast.
      notify.error({ message: t('resumeInput.downloadFailed') });
    }
  };

  const handleFileChange = async (file: File) => {
    clearReview();
    if (file.size > MAX_BYTES) {
      notify.error({ message: t('resumeInput.tooLarge') });
      return;
    }
    try {
      // importFile already saves the doc to the library and sets the review.
      const result = await importFile(file);
      const id = result?.id;
      if (id) {
        const text = await queryClient.fetchQuery({
          queryKey: keys.documents.text(id),
          queryFn: () => api.documents.getText(id),
        });
        onChange(text);
        setSelectedDocId(id);
        setExpanded(false);
      }
    } catch {
      notify.error({ message: t('resumeInput.saveFailed') });
    }
  };

  /** Save the current pasted/edited text to the library as a .txt document. */
  const handleSavePaste = async () => {
    if (!value.trim()) return;
    try {
      const firstLine =
        value
          .trim()
          .split('\n')
          .find((l) => l.trim())
          ?.trim()
          .slice(0, 40) || 'pasted-resume';
      const blob = new File([new TextEncoder().encode(value)], `${firstLine}.txt`, {
        type: 'text/plain',
      });
      const result = await importFile(blob);
      if (result?.id) {
        setSelectedDocId(result.id);
        setExpanded(false);
        notify.success({ message: t('resumeInput.savedToLibrary') });
      }
    } catch {
      notify.error({ message: t('resumeInput.saveFailed') });
    }
  };

  const profileUrlValid = isSupportedProfileUrl(profileUrl);

  const handleProfileUrlSubmit = async () => {
    const url = profileUrl.trim();
    if (!url || !profileUrlValid) return;
    try {
      const result = await profileImport.mutateAsync(url);
      if ('error' in result) {
        notify.error({
          message: isProfileAuthError(result.error)
            ? t('resumeInput.profileLoginRequired')
            : result.error,
        });
        return;
      }
      onChange(result.text);
      setSelectedDocId(null);
      setProfileUrl('');
      setShowUrlInput(false);
      setExpanded(false);
      notify.success({ message: t('resumeInput.profileImported') });
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : t('resumeInput.profileImportFailed'),
      });
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
    profileUrlValid,
    profileImportPending: profileImport.isPending,
    uploading: isPending,
    scanning: isOcr,
    openSavedMenu,
    handleSelectSaved,
    handleSetDefaultSaved,
    handleRemove,
    handleDownload,
    handleFileChange,
    handleSavePaste,
    handleProfileUrlSubmit,
    toggleUrlInput,
    review,
    clearReview,
  };
}
