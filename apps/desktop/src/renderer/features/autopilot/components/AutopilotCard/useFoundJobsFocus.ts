import { type RefObject, useCallback, useEffect, useRef, useState } from 'react';

interface Options {
  /** When true (tray/deep-link focus), auto-expand found-jobs + scroll into view. */
  focused?: boolean;
  /** A specific found-job url to scroll+highlight once expanded. */
  focusedJobUrl?: string | null;
  /** Called once the focus has been consumed, so the page can clear it. */
  onFocusHandled?: () => void;
  headerRef: RefObject<HTMLDivElement | null>;
  listContainerRef: RefObject<HTMLDivElement | null>;
  setShowFound: React.Dispatch<React.SetStateAction<boolean>>;
}

/**
 * Tray "New jobs" / deep-link focus: open this card's found-jobs and scroll to
 * it, then tell the page to clear the focus so a later click re-triggers.
 * Returns the transient highlight target plus `resolvePendingScroll`, which the
 * found-jobs panel calls when its expand animation completes.
 */
export function useFoundJobsFocus({
  focused,
  focusedJobUrl,
  onFocusHandled,
  headerRef,
  listContainerRef,
  setShowFound,
}: Options) {
  // Scroll-to-row + transient highlight target for `focusedJobUrl` (returning
  // from an Apply via Back). Kept in a ref (not state) since it isn't rendered;
  // `resolvePendingScroll` below clears it first so it can never double-fire
  // between the enter-animation and already-expanded-rAF paths.
  const pendingScrollUrlRef = useRef<string | null>(null);
  const pendingScrollRafRef = useRef<number | null>(null);
  const [highlightedUrl, setHighlightedUrl] = useState<string | null>(null);
  const highlightTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (highlightTimeoutRef.current) clearTimeout(highlightTimeoutRef.current);
      if (pendingScrollRafRef.current !== null) cancelAnimationFrame(pendingScrollRafRef.current);
    };
  }, []);

  // Idempotent: reads + clears `pendingScrollUrlRef` FIRST, so it's safe to
  // call from both the enter-animation completion and the already-expanded
  // rAF fallback below without double-scrolling or double-firing onFocusHandled.
  const resolvePendingScroll = useCallback(() => {
    const url = pendingScrollUrlRef.current;
    if (!url) return;
    pendingScrollUrlRef.current = null;
    const el = listContainerRef.current?.querySelector(`[data-job-url="${CSS.escape(url)}"]`);
    el?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    setHighlightedUrl(url);
    if (highlightTimeoutRef.current) clearTimeout(highlightTimeoutRef.current);
    highlightTimeoutRef.current = setTimeout(() => setHighlightedUrl(null), 1500);
    onFocusHandled?.();
  }, [listContainerRef, onFocusHandled]);

  // When `focusedJobUrl` is set (returning from an Apply via Back), defer the
  // scroll+highlight to that specific row: normally via the found-jobs panel's
  // `onAnimationComplete` once its expand animation finishes, or — if the panel
  // was ALREADY expanded, so no enter animation fires — via a rAF fallback here
  // so the focus can never wedge.
  useEffect(() => {
    if (!focused) return;
    if (focusedJobUrl) {
      pendingScrollUrlRef.current = focusedJobUrl;
      // Functional update: reads the PRE-focus `showFound` without adding it as
      // a dependency (adding it would re-run this effect — and re-force the
      // panel open — on every manual toggle while still focused).
      setShowFound((wasExpanded) => {
        if (wasExpanded) pendingScrollRafRef.current = requestAnimationFrame(resolvePendingScroll);
        return true;
      });
    } else {
      setShowFound(true);
      headerRef.current?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      onFocusHandled?.();
    }
  }, [focused, focusedJobUrl, headerRef, onFocusHandled, resolvePendingScroll, setShowFound]);

  return { highlightedUrl, resolvePendingScroll };
}
