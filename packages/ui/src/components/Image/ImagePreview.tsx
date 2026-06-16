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
  /** Allow drag-to-pan. Default true. */
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
 * reset, drag-to-pan, and prev/next across multiple items. Rendered in a portal on
 * the document body; closes on Escape or a backdrop click. Used by {@link Image}
 * (single item) and the preview group (many).
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

  // Lock body scroll + wire Escape / arrow keys while open.
  useEffect(() => {
    if (!open) return;
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onOpenChange(false);
      else if (hasNav && e.key === 'ArrowLeft') onIndexChange((index - 1 + total) % total);
      else if (hasNav && e.key === 'ArrowRight') onIndexChange((index + 1) % total);
    };
    window.addEventListener('keydown', onKey);
    return () => {
      document.body.style.overflow = prevOverflow;
      window.removeEventListener('keydown', onKey);
    };
  }, [open, hasNav, index, total, onOpenChange, onIndexChange]);

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
      role="dialog"
      aria-modal="true"
      onClick={() => onOpenChange(false)}
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
