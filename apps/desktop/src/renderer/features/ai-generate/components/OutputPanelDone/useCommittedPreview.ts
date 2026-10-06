import { useCallback, useEffect, useRef, useState } from 'react';

import { useDebouncedCommit } from '@/hooks/use-debounced-commit';

/**
 * Committed preview text, decoupled from the live edited string: the preview
 * renders COMMITTED text; local edits auto-commit after ~700 ms via
 * useDebouncedCommit; generation/regeneration commits immediately.
 */
export function useCommittedPreview(
  resumeOut: string,
  coverOut: string,
  activeOut: 'resume' | 'cover',
  onOutputChange: (value: string) => void
) {
  const [committedResume, setCommittedResume] = useState(resumeOut);
  const [committedCover, setCommittedCover] = useState(coverOut);
  // Track whether there is a pending debounced commit (drives the "Updating…" hint).
  const [pendingResume, setPendingResume] = useState(false);
  const [pendingCover, setPendingCover] = useState(false);

  // The last value the editor emitted per doc — distinguishes external changes
  // (generation / regeneration) from local edits, so external changes bypass the
  // debounce and refresh the preview immediately.
  const lastEditRef = useRef<{ resume: string | null; cover: string | null }>({
    resume: null,
    cover: null,
  });

  const setCommitted = useCallback((out: 'resume' | 'cover', text: string) => {
    if (out === 'resume') {
      setCommittedResume(text);
      setPendingResume(false);
    } else {
      setCommittedCover(text);
      setPendingCover(false);
    }
  }, []);

  const { scheduleCommit, flush, cancel } = useDebouncedCommit<'resume' | 'cover'>(setCommitted);

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

  // Auto-refresh committed when resumeOut/coverOut change for a reason OTHER than
  // a local edit (generation, regeneration). A local edit sets lastEditRef so the
  // debounce handles it instead.
  useEffect(() => {
    if (resumeOut !== lastEditRef.current.resume) {
      setCommittedResume(resumeOut);
      setPendingResume(false);
      lastEditRef.current.resume = null;
    }
  }, [resumeOut]);
  useEffect(() => {
    if (coverOut !== lastEditRef.current.cover) {
      setCommittedCover(coverOut);
      setPendingCover(false);
      lastEditRef.current.cover = null;
    }
  }, [coverOut]);

  // Record the emitted value as a local edit, schedule the debounced commit, and
  // propagate the canonical string (copy/export stay live without recompiling).
  // Pass (activeOut, value) so the pair is captured now — tab switches can't
  // misroute the commit to a different doc.
  const handleOutputChange = useCallback(
    (value: string) => {
      lastEditRef.current[activeOut] = value;
      if (activeOut === 'resume') setPendingResume(true);
      else setPendingCover(true);
      scheduleCommit(activeOut, value);
      onOutputChange(value);
    },
    [activeOut, scheduleCommit, onOutputChange]
  );

  const handleBlur = useCallback(() => {
    // flush() commits the (out, value) pair captured at scheduleCommit time —
    // uses the typed value, never the prop, and always routes to the correct doc.
    flush();
  }, [flush]);

  return {
    committed: activeOut === 'resume' ? committedResume : committedCover,
    isPending: activeOut === 'resume' ? pendingResume : pendingCover,
    handleOutputChange,
    handleBlur,
  };
}
