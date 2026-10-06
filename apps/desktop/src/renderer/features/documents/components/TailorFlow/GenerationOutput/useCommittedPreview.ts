import { useCallback, useEffect, useRef, useState } from 'react';

import { useDebouncedCommit } from '@/hooks/use-debounced-commit';

/**
 * Committed text per doc — what PdfPreview renders. Local edits auto-commit
 * after ~700 ms via useDebouncedCommit; generation/regeneration commits immediately.
 */
export function useCommittedPreview(
  activeOut: 'resume' | 'cover',
  output: string,
  onEdit: (text: string) => void
) {
  const [committed, setCommitted] = useState<Record<'resume' | 'cover', string>>({
    resume: activeOut === 'resume' ? output : '',
    cover: activeOut === 'cover' ? output : '',
  });
  const [pending, setPending] = useState(false);

  const lastEditRef = useRef<Record<'resume' | 'cover', string | null>>({
    resume: null,
    cover: null,
  });

  const commitToDoc = useCallback((out: 'resume' | 'cover', text: string) => {
    setCommitted((c) => ({ ...c, [out]: text }));
    setPending(false);
  }, []);

  const { scheduleCommit, flush, cancel } = useDebouncedCommit<'resume' | 'cover'>(commitToDoc);

  // Flush on doc/tab switch so a pending edit commits to ITS OWN doc before the
  // view changes. flush() uses the (out, value) pair captured at scheduleCommit
  // time — never the current activeOut — so the edit always lands in the right doc.
  const prevActiveOutRef = useRef(activeOut);
  useEffect(() => {
    if (prevActiveOutRef.current !== activeOut) {
      flush();
      prevActiveOutRef.current = activeOut;
    }
  }, [activeOut, flush]);

  // Cancel on unmount.
  useEffect(() => cancel, [cancel]);

  // Refresh committed when `output` changes for a reason OTHER than a local edit
  // (generation, regenerate, or tab switch). A local edit sets lastEditRef so the
  // debounce handles it instead.
  useEffect(() => {
    if (output !== lastEditRef.current[activeOut]) {
      setCommitted((c) => ({ ...c, [activeOut]: output }));
      setPending(false);
      lastEditRef.current[activeOut] = null;
    }
  }, [output, activeOut]);

  const handleEdit = useCallback(
    (value: string) => {
      lastEditRef.current[activeOut] = value;
      setPending(true);
      // Capture (activeOut, value) pair now — tab switches can't misroute the commit.
      scheduleCommit(activeOut, value);
      onEdit(value);
    },
    [activeOut, scheduleCommit, onEdit]
  );

  // flush() commits the (out, value) pair captured at scheduleCommit time —
  // uses the typed value, never the prop, and always routes to the correct doc.
  const handleBlur = useCallback(() => {
    flush();
  }, [flush]);

  return { committed, pending, handleEdit, handleBlur };
}
