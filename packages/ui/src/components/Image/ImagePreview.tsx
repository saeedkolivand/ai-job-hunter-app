import {
  ChevronLeft,
  ChevronRight,
  FlipHorizontal2,
  FlipVertical2,
  RefreshCw,
  RotateCcw,
  RotateCw,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import { type ReactNode, useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { Button } from '../Button';

/** The live view transform applied to the previewed image. */
export interface ImageTransform {
  scale: number;
  rotate: number;
  flipX: boolean;
  flipY: boolean;
  x: number;
  y: number;
}

const IDENTITY: ImageTransform = { scale: 1, rotate: 0, flipX: false, flipY: false, x: 0, y: 0 };

/** Pixels moved per arrow-key press — the keyboard equivalent of one drag step. */
const PAN_STEP = 40;

/**
 * Arrow-key pan deltas. Signs mirror dragging: ArrowRight moves the image to
 * the right exactly as a rightward drag does, so the two gestures agree.
 */
const PAN_KEYS: Record<string, { x: number; y: number } | undefined> = {
  ArrowLeft: { x: -PAN_STEP, y: 0 },
  ArrowRight: { x: PAN_STEP, y: 0 },
  ArrowUp: { x: 0, y: -PAN_STEP },
  ArrowDown: { x: 0, y: PAN_STEP },
};

export interface ImagePreviewProps {
  /** The previewable srcs; length > 1 enables prev/next navigation. */
  items: string[];
  /** Index of the currently shown item. */
  index: number;
  open: boolean;
  alt?: string;
  /** Multiplier applied per zoom step. Default 0.5. */
  scaleStep?: number;
  /** Minimum / maximum zoom. Defaults 1 / 50. */
  minScale?: number;
  maxScale?: number;
  /** Allow panning — pointer drag, or arrow keys while zoomed. Default true. */
  movable?: boolean;
  onIndexChange: (index: number) => void;
  onOpenChange: (open: boolean) => void;
  /** Replace the rendered image node (receives the default node + live transform). */
  imageRender?: (
    node: ReactNode,
    info: { transform: ImageTransform; current: number }
  ) => ReactNode;
  /** Replace the toolbar (receives the default toolbar node). */
  toolbarRender?: (
    node: ReactNode,
    info: { transform: ImageTransform; current: number; total: number }
  ) => ReactNode;
}

/**
 * Full-screen image lightbox: zoom (buttons + wheel + double-click), rotate, flip,
 * reset, pan (drag or arrow keys while zoomed), and prev/next across multiple
 * items. Rendered in a portal on the document body; closes on Escape or a
 * backdrop click. Used by {@link Image} (single item) and the preview group (many).
 */
export function ImagePreview({
  items,
  index,
  open,
  alt,
  scaleStep = 0.5,
  minScale = 1,
  maxScale = 50,
  movable = true,
  onIndexChange,
  onOpenChange,
  imageRender,
  toolbarRender,
}: ImagePreviewProps) {
  const [transform, setTransform] = useState<ImageTransform>(IDENTITY);
  const dialogRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ startX: number; startY: number; baseX: number; baseY: number } | null>(
    null
  );

  const total = items.length;
  const hasNav = total > 1;
  const src = items[index];

  const clampScale = useCallback(
    (s: number) => Math.min(maxScale, Math.max(minScale, s)),
    [minScale, maxScale]
  );

  // Reset the transform whenever the open item changes.
  useEffect(() => {
    if (open) setTransform(IDENTITY);
  }, [open, index]);

  // Move focus INTO the dialog when it opens, and hand it BACK on close. The
  // arrow keys are bound on `window`, so leaving focus on whatever opened the
  // preview means an arrow press both pans the image and reaches the control
  // behind it (a text field would move its caret, a list would change
  // selection). Focusing the dialog also gives a keyboard user somewhere to tab
  // FROM, per the APG dialog pattern; `tabIndex={-1}` makes it programmatically
  // focusable only.
  //
  // Taking focus without returning it is only half of that pattern: the dialog
  // unmounts on Escape/close, focus falls to `<body>`, and the next Tab
  // restarts at the top of the page instead of at the thumbnail the user opened
  // (WCAG 2.4.3). `isConnected` guards the case where the opener itself is gone
  // by then — a preview closed by a re-render that also removed its trigger.
  useEffect(() => {
    if (!open) return;
    const opener = document.activeElement as HTMLElement | null;
    dialogRef.current?.focus();
    return () => {
      if (opener?.isConnected) opener.focus();
    };
  }, [open]);

  // Lock body scroll while open.
  useEffect(() => {
    if (!open) return;
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      document.body.style.overflow = prevOverflow;
    };
  }, [open]);

  // Every other transform (zoom/rotate/flip/reset) has a toolbar button; panning
  // was pointer-only, so a keyboard user could zoom in and never reach the rest
  // of the image (WCAG 2.1.1 / 2.5.7). While zoomed the arrows pan; at 1x — where
  // there is nothing to pan — they keep stepping through the items.
  const zoomed = movable && transform.scale > 1;
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onOpenChange(false);
        return;
      }
      const pan = PAN_KEYS[e.key];
      if (pan && zoomed) {
        e.preventDefault();
        setTransform((p) => ({ ...p, x: p.x + pan.x, y: p.y + pan.y }));
        return;
      }
      if (!hasNav) return;
      if (e.key === 'ArrowLeft') onIndexChange((index - 1 + total) % total);
      else if (e.key === 'ArrowRight') onIndexChange((index + 1) % total);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, hasNav, index, total, zoomed, onOpenChange, onIndexChange]);

  if (!open || !src || typeof document === 'undefined') return null;

  const zoom = (factor: number) =>
    setTransform((p) => ({ ...p, scale: clampScale(p.scale * factor) }));
  const rotate = (deg: number) => setTransform((p) => ({ ...p, rotate: p.rotate + deg }));
  const prev = () => onIndexChange((index - 1 + total) % total);
  const next = () => onIndexChange((index + 1) % total);

  const onPointerDown = (e: React.PointerEvent) => {
    if (!movable) return;
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
    dragRef.current = {
      startX: e.clientX,
      startY: e.clientY,
      baseX: transform.x,
      baseY: transform.y,
    };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = dragRef.current;
    if (!d) return;
    setTransform((p) => ({
      ...p,
      x: d.baseX + (e.clientX - d.startX),
      y: d.baseY + (e.clientY - d.startY),
    }));
  };
  const onPointerUp = () => {
    dragRef.current = null;
  };

  const sx = transform.scale * (transform.flipX ? -1 : 1);
  const sy = transform.scale * (transform.flipY ? -1 : 1);

  const imgNode: ReactNode = (
    <img
      key={src}
      src={src}
      alt={alt ?? ''}
      draggable={false}
      onClick={(e) => e.stopPropagation()}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onDoubleClick={() =>
        setTransform((p) => (p.scale > 1 ? IDENTITY : { ...p, scale: clampScale(2) }))
      }
      style={{
        transform: `translate3d(${transform.x}px, ${transform.y}px, 0) rotate(${transform.rotate}deg) scale(${sx}, ${sy})`,
        cursor: movable ? 'grab' : 'default',
        transition: dragRef.current ? 'none' : 'transform 150ms ease-out',
      }}
      className="max-h-[85vh] max-w-[90vw] select-none object-contain"
    />
  );

  const action = (label: string, icon: ReactNode, onClick: () => void) => (
    <Button
      variant="unstyled"
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="flex h-8 w-8 items-center justify-center rounded-lg text-white/70 transition-colors hover:bg-white/10 hover:text-white"
    >
      {icon}
    </Button>
  );

  const toolbarNode: ReactNode = (
    <div
      onClick={(e) => e.stopPropagation()}
      className="absolute bottom-6 left-1/2 flex -translate-x-1/2 items-center gap-1 rounded-full border border-white/10 bg-black/50 px-2 py-1.5 backdrop-blur"
    >
      {action('Zoom out', <ZoomOut size={16} />, () => zoom(1 / (1 + scaleStep)))}
      {action('Zoom in', <ZoomIn size={16} />, () => zoom(1 + scaleStep))}
      {action('Rotate left', <RotateCcw size={16} />, () => rotate(-90))}
      {action('Rotate right', <RotateCw size={16} />, () => rotate(90))}
      {action('Flip horizontal', <FlipHorizontal2 size={16} />, () =>
        setTransform((p) => ({ ...p, flipX: !p.flipX }))
      )}
      {action('Flip vertical', <FlipVertical2 size={16} />, () =>
        setTransform((p) => ({ ...p, flipY: !p.flipY }))
      )}
      {action('Reset', <RefreshCw size={16} />, () => setTransform(IDENTITY))}
    </div>
  );

  return createPortal(
    <div
      ref={dialogRef}
      role="dialog"
      aria-modal="true"
      tabIndex={-1}
      // Panning has no toolbar button, so the arrow keys are the ONLY way to
      // reach the rest of a zoomed image — that makes them part of the dialog's
      // name/role/value story, not an undocumented extra.
      aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"
      onClick={() => onOpenChange(false)}
      // No `outline-none` here: it would be inert. The global focus ring in
      // `utilities.css` is UNLAYERED, so a Tailwind utility (layer `utilities`)
      // cannot override it — the composite-container rule next to that ring is
      // what keeps a 2px outline off the full-viewport backdrop.
      className="fixed inset-0 z-[1000] flex items-center justify-center bg-black/80"
    >
      <Button
        variant="unstyled"
        type="button"
        aria-label="Close"
        title="Close"
        onClick={(e) => {
          e.stopPropagation();
          onOpenChange(false);
        }}
        className="absolute right-4 top-4 flex h-9 w-9 items-center justify-center rounded-full text-white/70 transition-colors hover:bg-white/10 hover:text-white"
      >
        <X size={18} />
      </Button>

      {hasNav && (
        <>
          <Button
            variant="unstyled"
            type="button"
            aria-label="Previous"
            onClick={(e) => {
              e.stopPropagation();
              prev();
            }}
            className="absolute left-4 top-1/2 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full text-white/70 transition-colors hover:bg-white/10 hover:text-white"
          >
            <ChevronLeft size={22} />
          </Button>
          <Button
            variant="unstyled"
            type="button"
            aria-label="Next"
            onClick={(e) => {
              e.stopPropagation();
              next();
            }}
            className="absolute right-4 top-1/2 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full text-white/70 transition-colors hover:bg-white/10 hover:text-white"
          >
            <ChevronRight size={22} />
          </Button>
          <div className="absolute left-1/2 top-5 -translate-x-1/2 rounded-full bg-black/50 px-2.5 py-1 text-xs text-white/80">
            {index + 1} / {total}
          </div>
        </>
      )}

      {imageRender ? imageRender(imgNode, { transform, current: index }) : imgNode}
      {toolbarRender
        ? toolbarRender(toolbarNode, { transform, current: index, total })
        : toolbarNode}
    </div>,
    document.body
  );
}
