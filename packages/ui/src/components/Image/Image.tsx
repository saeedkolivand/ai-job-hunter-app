import { Eye } from 'lucide-react';
import { type CSSProperties, type ReactNode, useContext, useEffect, useState } from 'react';

import { cn } from '../../lib/cn';
import { Button } from '../Button';
import { PreviewGroupContext } from './context';
import { ImagePreview, type ImagePreviewProps } from './ImagePreview';
import { PreviewGroup } from './PreviewGroup';

export interface ImagePreviewType {
  /** Controlled open state (single-image preview). */
  open?: boolean;
  /** Preview a different src than the thumbnail. */
  src?: string;
  /** Custom hover-mask content, or `false` to hide the mask. */
  mask?: ReactNode | false;
  movable?: boolean;
  scaleStep?: number;
  minScale?: number;
  maxScale?: number;
  onOpenChange?: (open: boolean) => void;
  imageRender?: ImagePreviewProps['imageRender'];
  toolbarRender?: ImagePreviewProps['toolbarRender'];
}

export interface ImageProps {
  src: string;
  alt?: string;
  width?: number | string;
  height?: number | string;
  /** A src shown when the image fails to load. */
  fallback?: string;
  /** Loading indicator: `true` for a default shimmer, or a custom node. */
  placeholder?: ReactNode | boolean;
  /** Enable/configure the click-to-zoom preview. Default `true`. */
  preview?: boolean | ImagePreviewType;
  className?: string;
  rootClassName?: string;
  style?: CSSProperties;
  onError?: (e: React.SyntheticEvent<HTMLImageElement, Event>) => void;
}

type LoadStatus = 'loading' | 'loaded' | 'error';

/**
 * An image with a built-in loading placeholder, an error fallback, and a
 * click-to-zoom preview lightbox (zoom / rotate / flip / drag). Wrap several in
 * {@link PreviewGroup} (`Image.PreviewGroup`) to share one viewer with prev/next.
 */
function ImageBase({
  src,
  alt,
  width,
  height,
  fallback,
  placeholder,
  preview = true,
  className,
  rootClassName,
  style,
  onError,
}: ImageProps) {
  const [status, setStatus] = useState<LoadStatus>('loading');
  const group = useContext(PreviewGroupContext);

  const previewConfig: ImagePreviewType = typeof preview === 'object' ? preview : {};
  const previewEnabled = preview !== false;

  const [localOpen, setLocalOpen] = useState(false);
  const open = previewConfig.open ?? localOpen;
  const setOpen = (o: boolean) => {
    setLocalOpen(o);
    previewConfig.onOpenChange?.(o);
  };

  // Reset load status when the source changes.
  useEffect(() => {
    setStatus('loading');
  }, [src]);

  // Register with an enclosing preview group (if any) for shared prev/next.
  const [groupId, setGroupId] = useState<number | null>(null);
  useEffect(() => {
    if (!group || !previewEnabled) return;
    const id = group.register(src);
    setGroupId(id);
    return () => group.unregister(id);
  }, [group, previewEnabled, src]);

  const isError = status === 'error';
  const displaySrc = isError && fallback ? fallback : src;
  const previewSrc = previewConfig.src ?? src;
  const showMask = previewEnabled && !isError && previewConfig.mask !== false;

  const openPreview = () => {
    if (!previewEnabled || isError) return;
    if (group && groupId != null) group.onPreview(groupId);
    else setOpen(true);
  };

  return (
    <div className={cn('relative inline-block overflow-hidden', rootClassName)}>
      {status === 'loading' && placeholder ? (
        <div className="absolute inset-0 z-[1] flex items-center justify-center bg-foreground/[0.04]">
          {placeholder === true ? (
            <span className="h-full w-full animate-pulse bg-foreground/10" />
          ) : (
            placeholder
          )}
        </div>
      ) : null}

      <img
        src={displaySrc}
        alt={alt ?? ''}
        onLoad={() => setStatus('loaded')}
        onError={(e) => {
          setStatus('error');
          onError?.(e);
        }}
        style={{ width, height, ...style }}
        className={cn('block', className)}
      />

      {showMask && (
        <Button
          variant="unstyled"
          type="button"
          aria-label="Preview image"
          onClick={openPreview}
          className="absolute inset-0 flex cursor-pointer items-center justify-center gap-1.5 bg-black/45 text-sm text-white opacity-0 transition-opacity hover:opacity-100 focus-visible:opacity-100"
        >
          {previewConfig.mask ?? (
            <>
              <Eye size={16} /> Preview
            </>
          )}
        </Button>
      )}

      {/* Single-image lightbox — a preview group owns its own shared one. */}
      {previewEnabled && !group && (
        <ImagePreview
          items={[previewSrc]}
          index={0}
          open={open}
          alt={alt}
          movable={previewConfig.movable}
          scaleStep={previewConfig.scaleStep}
          minScale={previewConfig.minScale}
          maxScale={previewConfig.maxScale}
          imageRender={previewConfig.imageRender}
          toolbarRender={previewConfig.toolbarRender}
          onIndexChange={() => undefined}
          onOpenChange={setOpen}
        />
      )}
    </div>
  );
}

/** {@link ImageBase} with the `PreviewGroup` sub-component attached. */
export const Image = Object.assign(ImageBase, { PreviewGroup });
