import { type ReactNode, useCallback, useMemo, useRef, useState } from 'react';

import { PreviewGroupContext } from './context';
import { ImagePreview, type ImagePreviewProps } from './ImagePreview';

export interface ImagePreviewGroupConfig {
  /** Controlled open state. */
  open?: boolean;
  /** Controlled current index. */
  current?: number;
  movable?: boolean;
  scaleStep?: number;
  minScale?: number;
  maxScale?: number;
  onChange?: (current: number) => void;
  onOpenChange?: (open: boolean) => void;
  imageRender?: ImagePreviewProps['imageRender'];
  toolbarRender?: ImagePreviewProps['toolbarRender'];
}

export interface ImagePreviewGroupProps {
  children?: ReactNode;
  /** Explicit src list — an alternative to wrapping `<Image>` children. */
  items?: string[];
  /** Disable (false) or configure the shared preview lightbox. */
  preview?: boolean | ImagePreviewGroupConfig;
}

/**
 * Groups several images behind one shared lightbox: clicking any child opens the
 * viewer at that image, with prev/next navigation across the whole set. Children
 * register their src on mount; an explicit `items` list can be supplied instead.
 */
export function PreviewGroup({ children, items, preview = true }: ImagePreviewGroupProps) {
  const config = typeof preview === 'object' ? preview : {};
  const enabled = preview !== false;
  const { onChange, onOpenChange } = config;

  const registry = useRef(new Map<number, string>());
  const counter = useRef(0);
  const [openState, setOpenState] = useState(false);
  const [indexState, setIndexState] = useState(0);

  const setOpen = useCallback(
    (o: boolean) => {
      setOpenState(o);
      onOpenChange?.(o);
    },
    [onOpenChange]
  );
  const setIndex = useCallback(
    (i: number) => {
      setIndexState(i);
      onChange?.(i);
    },
    [onChange]
  );

  const register = useCallback((src: string) => {
    const id = ++counter.current;
    registry.current.set(id, src);
    return id;
  }, []);
  const unregister = useCallback((id: number) => {
    registry.current.delete(id);
  }, []);
  const onPreview = useCallback(
    (id: number) => {
      if (!enabled) return;
      const pos = [...registry.current.keys()].indexOf(id);
      setIndex(pos < 0 ? 0 : pos);
      setOpen(true);
    },
    [enabled, setIndex, setOpen]
  );

  const ctx = useMemo(
    () => ({ register, unregister, onPreview }),
    [register, unregister, onPreview]
  );

  const previewItems = items ?? [...registry.current.values()];
  const open = config.open ?? openState;
  const current = config.current ?? indexState;

  return (
    <PreviewGroupContext.Provider value={ctx}>
      {children}
      {enabled && (
        <ImagePreview
          items={previewItems}
          index={current}
          open={open}
          movable={config.movable}
          scaleStep={config.scaleStep}
          minScale={config.minScale}
          maxScale={config.maxScale}
          imageRender={config.imageRender}
          toolbarRender={config.toolbarRender}
          onIndexChange={setIndex}
          onOpenChange={setOpen}
        />
      )}
    </PreviewGroupContext.Provider>
  );
}
