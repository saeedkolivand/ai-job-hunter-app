import { useLayoutEffect, useState } from 'react';

/** Matches the panel's `w-[22rem]` — clamps left-edge overflow when the trigger
 *  sits near a viewport edge (anchored-portal mode only). */
const POPOVER_WIDTH_PX = 352;

/**
 * Anchored-portal mode only (`anchorEl` set): fixed-position the panel below-right of
 * the trigger. Returns `undefined` for the inline placement.
 */
export function useAnchoredStyle(
  anchorEl: HTMLElement | null | undefined,
  panelRef: React.RefObject<HTMLDivElement | null>
): React.CSSProperties | undefined {
  const [anchorRect, setAnchorRect] = useState<DOMRect | null>(null);
  // The popover's own rendered height — it varies with content (streaming
  // result, error text, …), so it can't be assumed static. Used to decide
  // whether the panel fits below the trigger or must flip above it.
  const [panelHeight, setPanelHeight] = useState<number | null>(null);

  // Anchored-portal mode only: measure the trigger on open, and re-measure on
  // scroll/resize — the caller's scrollable ancestor (e.g. a modal body) can move
  // the trigger while this fixed-positioned popover stays put otherwise. Also
  // track the panel's own height via ResizeObserver so a later content change
  // (e.g. the streaming result appearing) can re-trigger the fit check below.
  useLayoutEffect(() => {
    if (!anchorEl) return;
    const measureAnchor = () => setAnchorRect(anchorEl.getBoundingClientRect());
    measureAnchor();
    window.addEventListener('scroll', measureAnchor, true);
    window.addEventListener('resize', measureAnchor);

    const panel = panelRef.current;
    let observer: ResizeObserver | undefined;
    if (panel) {
      setPanelHeight(panel.getBoundingClientRect().height);
      observer = new ResizeObserver(() => setPanelHeight(panel.getBoundingClientRect().height));
      observer.observe(panel);
    }

    return () => {
      window.removeEventListener('scroll', measureAnchor, true);
      window.removeEventListener('resize', measureAnchor);
      observer?.disconnect();
    };
  }, [anchorEl, panelRef]);

  // Anchored-portal mode: fixed-position below-right of the trigger, clamped so
  // the (fixed-width) panel never runs off the left edge, and flipped ABOVE the
  // trigger when it wouldn't fit below (e.g. Rewrite opened on a question near
  // the bottom of a scrollable, height-capped modal) — clamped so it also never
  // runs off the top edge. `visibility: hidden` (not `display: none`) until the
  // first measurement lands keeps the panel laid out (so its real height can be
  // read) without a visible flash — it's gone by paint since `useLayoutEffect`
  // flushes `setAnchorRect`/`setPanelHeight` before the browser paints.
  return anchorEl
    ? anchorRect
      ? {
          position: 'fixed',
          top:
            panelHeight !== null && anchorRect.bottom + panelHeight + 4 > window.innerHeight - 8
              ? Math.max(8, anchorRect.top - panelHeight - 4)
              : anchorRect.bottom + 4,
          left: Math.min(
            Math.max(8, anchorRect.right - POPOVER_WIDTH_PX),
            window.innerWidth - POPOVER_WIDTH_PX - 8
          ),
        }
      : { position: 'fixed', top: 0, left: 0, visibility: 'hidden' }
    : undefined;
}
